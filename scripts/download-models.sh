#!/bin/bash

# Hush Model Downloader Script
# Downloads common Whisper models for offline use

# set -e removed for safer handling of complex functions

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MODELS_DIR="$PROJECT_ROOT/models"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Available models with their sizes
declare -A MODEL_SIZES=(
    ["tiny"]="39MB"
    ["base"]="74MB"
    ["small"]="244MB"
    ["medium"]="769MB"
    ["large"]="1.55GB"
    ["large-v3"]="1.55GB"
)

print_usage() {
    echo "Usage: $0 [OPTIONS] [MODEL_SIZE]"
    echo
    echo "Download Whisper models for Hush voice-to-text."
    echo
    echo "Available models:"
    for model in "${!MODEL_SIZES[@]}"; do
        echo "  $model (${MODEL_SIZES[$model]})"
    done | sort
    echo
    echo "Options:"
    echo "  -h, --help     Show this help message"
    echo "  -l, --list     List available and cached models"
    echo "  -a, --all      Download all models"
    echo "  --info         Show cache information"
    echo "  --clear        Clear model cache"
    echo "  --models-dir   Set custom models directory"
    echo
    echo "Examples:"
    echo "  $0 tiny           # Download tiny model"
    echo "  $0 --all          # Download all models"
    echo "  $0 --list         # List models"
    echo "  $0 --info         # Show cache info"
}

check_dependencies() {
    if ! command -v curl >/dev/null 2>&1; then
        echo -e "${RED}Error: curl not found. Please install curl.${NC}" >&2
        exit 1
    fi
}

# Model download URLs
declare -A MODEL_URLS=(
    ["tiny"]="https://huggingface.co/openai/whisper-tiny/resolve/main/pytorch_model.bin"
    ["base"]="https://huggingface.co/openai/whisper-base/resolve/main/pytorch_model.bin"
    ["small"]="https://huggingface.co/openai/whisper-small/resolve/main/pytorch_model.bin"
    ["medium"]="https://huggingface.co/openai/whisper-medium/resolve/main/pytorch_model.bin"
    ["large"]="https://huggingface.co/openai/whisper-large-v3/resolve/main/pytorch_model.bin"
    ["large-v3"]="https://huggingface.co/openai/whisper-large-v3/resolve/main/pytorch_model.bin"
)

get_model_filename() {
    local model="$1"
    if [[ "$model" == "large-v3" ]]; then
        echo "whisper-large-v3.bin"
    else
        echo "whisper-$model.bin"
    fi
}

download_model() {
    local model="$1"
    
    if [[ -z "$model" ]]; then
        echo -e "${RED}Error: No model specified${NC}" >&2
        return 1
    fi
    
    if [[ ! -v MODEL_SIZES["$model"] || ! -v MODEL_URLS["$model"] ]]; then
        echo -e "${RED}Error: Unknown model '$model'${NC}" >&2
        echo "Available models: ${!MODEL_SIZES[*]}"
        return 1
    fi
    
    local filename
    filename=$(get_model_filename "$model")
    local model_path="$MODELS_DIR/$filename"
    
    if [[ -f "$model_path" ]]; then
        echo -e "${GREEN}Model $model is already cached at $model_path${NC}"
        return 0
    fi
    
    # Ensure models directory exists
    mkdir -p "$MODELS_DIR"
    
    echo -e "${BLUE}Downloading $model model (${MODEL_SIZES[$model]})...${NC}"
    echo "This may take a while..."
    
    if ! command -v curl >/dev/null 2>&1; then
        echo -e "${RED}Error: curl not found. Please install curl.${NC}" >&2
        return 1
    fi
    
    if curl -L --progress-bar -o "$model_path" "${MODEL_URLS[$model]}"; then
        local file_size
        file_size=$(stat -c%s "$model_path" 2>/dev/null || echo "0")
        local size_mb
        size_mb=$(echo "scale=1; $file_size / 1000000" | bc 2>/dev/null || echo "unknown")
        echo -e "${GREEN}✓ Successfully downloaded $model model ($size_mb MB)${NC}"
    else
        echo -e "${RED}✗ Failed to download $model model${NC}" >&2
        [[ -f "$model_path" ]] && rm -f "$model_path"  # Clean up partial download
        return 1
    fi
}

list_models() {
    echo "Available Whisper models:"
    echo
    
    local cached_count=0
    local total_count=${#MODEL_SIZES[@]}
    
    # Use fixed order for consistent output
    for model in tiny base small medium large large-v3; do
        local filename
        filename=$(get_model_filename "$model")
        local model_path="$MODELS_DIR/$filename"
        
        if [[ -f "$model_path" ]]; then
            echo -e "  ${GREEN}✓ cached${NC}   $model - (${MODEL_SIZES[$model]})"
            ((cached_count++))
        else
            echo -e "    not cached $model - (${MODEL_SIZES[$model]})"
        fi
    done
    
    echo
    echo "Cached: $cached_count/$total_count models"
}

show_info() {
    echo "Model Cache Information:"
    echo
    echo "Cache directory: $MODELS_DIR"
    
    local total_size=0
    local cached_models=()
    
    for model in "${!MODEL_SIZES[@]}"; do
        local filename
        filename=$(get_model_filename "$model")
        local model_path="$MODELS_DIR/$filename"
        
        if [[ -f "$model_path" ]]; then
            local file_size
            file_size=$(stat -c%s "$model_path" 2>/dev/null || echo "0")
            total_size=$((total_size + file_size))
            cached_models+=("$model")
        fi
    done
    
    local total_mb
    total_mb=$(echo "scale=1; $total_size / 1000000" | bc 2>/dev/null || echo "unknown")
    echo "Total cache size: $total_mb MB"
    echo "Cached models: ${#cached_models[@]}"
    
    if [[ ${#cached_models[@]} -gt 0 ]]; then
        echo
        echo "Cached models:"
        for model in "${cached_models[@]}"; do
            local filename
            filename=$(get_model_filename "$model")
            local model_path="$MODELS_DIR/$filename"
            local file_size
            file_size=$(stat -c%s "$model_path" 2>/dev/null || echo "0")
            local size_mb
            size_mb=$(echo "scale=1; $file_size / 1000000" | bc 2>/dev/null || echo "unknown")
            echo "  $model - $size_mb MB"
        done | sort
    fi
}

clear_cache() {
    if [[ ! -d "$MODELS_DIR" ]]; then
        echo "Cache directory doesn't exist."
        return 0
    fi
    
    # Calculate current cache size
    local total_size=0
    local file_count=0
    
    for model in "${!MODEL_SIZES[@]}"; do
        local filename
        filename=$(get_model_filename "$model")
        local model_path="$MODELS_DIR/$filename"
        
        if [[ -f "$model_path" ]]; then
            local file_size
            file_size=$(stat -c%s "$model_path" 2>/dev/null || echo "0")
            total_size=$((total_size + file_size))
            ((file_count++))
        fi
    done
    
    if [[ $file_count -eq 0 ]]; then
        echo "Cache is already empty."
        return 0
    fi
    
    local total_mb
    total_mb=$(echo "scale=1; $total_size / 1000000" | bc 2>/dev/null || echo "unknown")
    echo "This will delete $total_mb MB of cached models."
    echo "Are you sure? (y/N)"
    
    read -r reply
    if [[ ! $reply =~ ^[Yy]$ ]]; then
        echo "Cancelled."
        return 0
    fi
    
    # Remove cached model files
    for model in "${!MODEL_SIZES[@]}"; do
        local filename
        filename=$(get_model_filename "$model")
        local model_path="$MODELS_DIR/$filename"
        
        if [[ -f "$model_path" ]]; then
            rm -f "$model_path"
        fi
    done
    
    echo -e "${GREEN}✓ Cache cleared successfully.${NC}"
}

download_all_models() {
    echo -e "${BLUE}Downloading all Whisper models...${NC}"
    echo "This will download approximately 3.2GB of models."
    echo
    
    read -p "Continue? (y/N): " -n 1 -r
    echo
    
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Cancelled."
        return 0
    fi
    
    local failed_models=()
    
    for model in "${!MODEL_SIZES[@]}"; do
        if ! download_model "$model"; then
            failed_models+=("$model")
        fi
    done
    
    if [[ ${#failed_models[@]} -eq 0 ]]; then
        echo -e "${GREEN}✓ All models downloaded successfully!${NC}"
    else
        echo -e "${YELLOW}Some models failed to download: ${failed_models[*]}${NC}"
        return 1
    fi
}

main() {
    local models_dir_override=""
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                print_usage
                exit 0
                ;;
            -l|--list)
                list_models
                exit 0
                ;;
            -a|--all)
                check_dependencies
                download_all_models
                exit $?
                ;;
            --info)
                show_info
                exit 0
                ;;
            --clear)
                clear_cache
                exit 0
                ;;
            --models-dir)
                if [[ -n "$2" ]]; then
                    MODELS_DIR="$2"
                    shift 2
                else
                    echo -e "${RED}Error: --models-dir requires a path${NC}" >&2
                    exit 1
                fi
                ;;
            -*)
                echo -e "${RED}Error: Unknown option $1${NC}" >&2
                print_usage
                exit 1
                ;;
            *)
                # This should be a model name
                check_dependencies
                download_model "$1"
                exit $?
                ;;
        esac
    done
    
    # If no arguments provided, show usage
    print_usage
    exit 1
}

main "$@"