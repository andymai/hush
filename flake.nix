{
  description = "Hush: local voice-to-text for Linux desktops";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      inherit (pkgs) lib;

      # winit, glutin, GLFW, and the typing keymap load these with dlopen, so the linker does not record them.
      dlopenLibs = with pkgs; [
        libGL
        vulkan-loader
        wayland
        libxkbcommon
        libx11
        libxcb
        libxcursor
        libxext
        libxi
        libxinerama
        libxrandr
        libxrender
        libxxf86vm
      ];

      hush = pkgs.rustPlatform.buildRustPackage {
        pname = "hush";
        version = (lib.importTOML ./Cargo.toml).package.version;
        # Only the files the build reads, so a docs change does not rebuild Hush.
        src = lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions [
            ./Cargo.toml
            ./Cargo.lock
            ./build.rs
            ./src
            ./config
            ./packaging
          ];
        };
        cargoLock.lockFile = ./Cargo.lock;
        buildFeatures = [ "vulkan" ];

        nativeBuildInputs = with pkgs; [
          cmake
          pkg-config
          shaderc
        ];
        buildInputs = with pkgs; [
          alsa-lib
          vulkan-headers
          vulkan-loader
          spirv-headers
          libx11
          libxi
          libxtst
          libxcursor
          libxrandr
          libxinerama
          libGL
        ];

        # cmake is for transcribe.cpp inside the transcribe-cpp-sys build script, not for this source tree.
        dontUseCmakeConfigure = true;

        # transcribe-cpp-sys passes this to CMake, so the build never links a system BLAS.
        TRANSCRIBE_CMAKE_ARGS = "-DTRANSCRIBE_USE_SYSTEM_BLAS=OFF";

        # CI runs the tests. Some of them open audio and input devices that the build sandbox does not have.
        doCheck = false;

        # The default fixup cuts the RUNPATH down to DT_NEEDED and would drop dlopenLibs.
        dontPatchELF = true;

        postInstall = ''
          install -Dm644 packaging/io.github.andymai.hush.desktop -t $out/share/applications
          install -Dm644 packaging/io.github.andymai.hush.svg -t $out/share/icons/hicolor/scalable/apps
          install -Dm644 packaging/70-hush.rules -t $out/lib/udev/rules.d
          install -Dm644 packaging/hush.service -t $out/lib/systemd/user
          substituteInPlace $out/lib/systemd/user/hush.service --replace-fail /usr/bin/hush $out/bin/hush
          patchelf --add-rpath ${lib.makeLibraryPath dlopenLibs} $out/bin/hush
        '';

        meta = {
          description = "Local voice-to-text for Linux desktops: hold a key, speak, and the text appears at your cursor";
          homepage = "https://github.com/andymai/hush";
          license = lib.licenses.mit;
          mainProgram = "hush";
          platforms = [ system ];
        };
      };
    in
    {
      packages.${system} = {
        inherit hush;
        default = hush;
      };

      devShells.${system}.default = pkgs.mkShell {
        inputsFrom = [ hush ];
        packages = with pkgs; [
          clippy
          rustfmt
          cargo-nextest
        ];
        LD_LIBRARY_PATH = lib.makeLibraryPath dlopenLibs;
      };

      formatter.${system} = pkgs.nixfmt;
    };
}
