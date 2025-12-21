/// Entity recognition for proper capitalization
use std::collections::{HashMap, HashSet};
use tracing::debug;

/// Static lists of entities that need proper capitalization
static TECH_BRANDS: &[(&str, &str)] = &[
    // Programming languages
    ("javascript", "JavaScript"),
    ("typescript", "TypeScript"),
    ("golang", "Go"),
    ("csharp", "C#"),
    ("cplusplus", "C++"),
    ("postgresql", "PostgreSQL"),
    ("postgres", "PostgreSQL"),
    ("mysql", "MySQL"),
    ("mongodb", "MongoDB"),
    // Frameworks & libraries
    ("react", "React"),
    ("vue", "Vue"),
    ("angular", "Angular"),
    ("nextjs", "Next.js"),
    ("nodejs", "Node.js"),
    ("express", "Express"),
    ("django", "Django"),
    ("flask", "Flask"),
    ("fastapi", "FastAPI"),
    ("pytorch", "PyTorch"),
    ("tensorflow", "TensorFlow"),
    // Platforms & services
    ("github", "GitHub"),
    ("gitlab", "GitLab"),
    ("bitbucket", "Bitbucket"),
    ("aws", "AWS"),
    ("gcp", "GCP"),
    ("azure", "Azure"),
    ("vercel", "Vercel"),
    ("netlify", "Netlify"),
    ("heroku", "Heroku"),
    ("docker", "Docker"),
    ("kubernetes", "Kubernetes"),
    // Databases & tools
    ("redis", "Redis"),
    ("elasticsearch", "Elasticsearch"),
    ("kafka", "Kafka"),
    ("rabbitmq", "RabbitMQ"),
    ("graphql", "GraphQL"),
    ("grpc", "gRPC"),
    // Operating systems
    ("linux", "Linux"),
    ("ubuntu", "Ubuntu"),
    ("debian", "Debian"),
    ("macos", "macOS"),
    ("windows", "Windows"),
];

static ACRONYMS: &[&str] = &[
    "API", "REST", "CRUD", "HTTP", "HTTPS", "SSH", "FTP", "JSON", "XML", "HTML", "CSS", "SQL",
    "NoSQL", "IDE", "CLI", "GUI", "UI", "UX", "AWS", "GCP", "S3", "EC2", "RDS", "CI", "CD",
    "DevOps", "MLOps", "AI", "ML", "LLM", "NLP", "GPU", "CPU", "RAM", "URL", "URI", "JWT", "OAuth",
    "SAML", "TCP", "UDP", "DNS", "IP", "VPN", "YAML", "TOML", "CSV", "PDF",
];

static COMMON_TECH_TERMS: &[(&str, &str)] = &[
    ("api", "API"),
    ("rest", "REST"),
    ("crud", "CRUD"),
    ("json", "JSON"),
    ("html", "HTML"),
    ("css", "CSS"),
    ("sql", "SQL"),
    ("http", "HTTP"),
    ("https", "HTTPS"),
    ("url", "URL"),
    ("oauth", "OAuth"),
    ("jwt", "JWT"),
];

/// Entity recognizer for smart capitalization
pub struct EntityRecognizer {
    /// Tech brands/products and their proper capitalization
    brands: HashMap<String, String>,

    /// Known acronyms
    acronyms: HashSet<String>,

    /// User-defined custom entities (can be loaded from config)
    custom_entities: HashMap<String, String>,
}

impl EntityRecognizer {
    /// Create a new entity recognizer with default lists
    pub fn new() -> Self {
        let mut brands = HashMap::new();
        for (lowercase, proper) in TECH_BRANDS {
            brands.insert(lowercase.to_string(), proper.to_string());
        }
        for (lowercase, proper) in COMMON_TECH_TERMS {
            brands.insert(lowercase.to_string(), proper.to_string());
        }

        let acronyms: HashSet<String> = ACRONYMS.iter().map(|s| s.to_string()).collect();

        Self {
            brands,
            acronyms,
            custom_entities: HashMap::new(),
        }
    }

    /// Add custom entity mappings
    pub fn add_custom_entity(&mut self, lowercase: String, proper: String) {
        self.custom_entities
            .insert(lowercase.to_lowercase(), proper);
    }

    /// Load custom entities from a map
    pub fn load_custom_entities(&mut self, entities: HashMap<String, String>) {
        for (key, value) in entities {
            self.custom_entities.insert(key.to_lowercase(), value);
        }
    }

    /// Apply smart capitalization to text
    pub fn capitalize(&self, text: &str) -> String {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut result = Vec::new();

        for word in words {
            // Check if it's already properly capitalized (has capitals)
            if word.chars().any(|c| c.is_uppercase()) {
                result.push(word.to_string());
                continue;
            }

            let lowercase = word.to_lowercase();

            // Check custom entities first (highest priority)
            if let Some(proper) = self.custom_entities.get(&lowercase) {
                debug!("Capitalizing '{}' → '{}' (custom)", word, proper);
                result.push(proper.clone());
                continue;
            }

            // Check tech brands
            if let Some(proper) = self.brands.get(&lowercase) {
                debug!("Capitalizing '{}' → '{}' (brand)", word, proper);
                result.push(proper.clone());
                continue;
            }

            // Check if it's a known acronym (case-insensitive match)
            if self.acronyms.iter().any(|a| a.to_lowercase() == lowercase) {
                let proper = lowercase.to_uppercase();
                debug!("Capitalizing '{}' → '{}' (acronym)", word, proper);
                result.push(proper);
                continue;
            }

            // Default: preserve as-is
            result.push(word.to_string());
        }

        result.join(" ")
    }

    /// Check if a word is a known entity
    pub fn is_entity(&self, word: &str) -> bool {
        let lowercase = word.to_lowercase();
        self.custom_entities.contains_key(&lowercase)
            || self.brands.contains_key(&lowercase)
            || self.acronyms.iter().any(|a| a.to_lowercase() == lowercase)
    }
}

impl Default for EntityRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tech_brands() {
        let recognizer = EntityRecognizer::new();

        assert_eq!(recognizer.capitalize("github"), "GitHub");
        assert_eq!(recognizer.capitalize("postgresql"), "PostgreSQL");
        assert_eq!(recognizer.capitalize("javascript"), "JavaScript");
        assert_eq!(recognizer.capitalize("nodejs"), "Node.js");
    }

    #[test]
    fn test_acronyms() {
        let recognizer = EntityRecognizer::new();

        assert_eq!(recognizer.capitalize("api"), "API");
        assert_eq!(recognizer.capitalize("rest"), "REST");
        assert_eq!(recognizer.capitalize("json"), "JSON");
        assert_eq!(recognizer.capitalize("crud"), "CRUD");
    }

    #[test]
    fn test_mixed_text() {
        let recognizer = EntityRecognizer::new();

        let result = recognizer.capitalize("using github api with nodejs and postgresql");
        assert_eq!(result, "using GitHub API with Node.js and PostgreSQL");
    }

    #[test]
    fn test_preserves_existing_caps() {
        let recognizer = EntityRecognizer::new();

        // Should not modify words that already have capitals
        assert_eq!(recognizer.capitalize("MyClass"), "MyClass");
        assert_eq!(recognizer.capitalize("CamelCase"), "CamelCase");
    }

    #[test]
    fn test_custom_entities() {
        let mut recognizer = EntityRecognizer::new();
        recognizer.add_custom_entity("hush".to_string(), "Hush".to_string());
        recognizer.add_custom_entity("wisprflow".to_string(), "WisprFlow".to_string());

        assert_eq!(recognizer.capitalize("hush"), "Hush");
        assert_eq!(recognizer.capitalize("wisprflow"), "WisprFlow");
    }

    #[test]
    fn test_unknown_words_preserved() {
        let recognizer = EntityRecognizer::new();

        // Unknown words should be preserved as-is
        assert_eq!(recognizer.capitalize("hello world"), "hello world");
        assert_eq!(
            recognizer.capitalize("some random text"),
            "some random text"
        );
    }
}
