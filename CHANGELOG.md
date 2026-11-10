# Changelog

## [0.23.0](https://github.com/andymai/hush/compare/v0.22.0...v0.23.0) (2026-11-10)


### Features

* **llm:** rewrite the selected text with a style ([#292](https://github.com/andymai/hush/issues/292)) ([3a96f2b](https://github.com/andymai/hush/commit/3a96f2b89924c0ce900faa13e42fa27a37ebf0d4))

## [0.22.0](https://github.com/andymai/hush/compare/v0.21.0...v0.22.0) (2026-11-10)


### Features

* **text:** add writing styles and a style for one take ([#290](https://github.com/andymai/hush/issues/290)) ([efe2266](https://github.com/andymai/hush/commit/efe2266b70d39fec555332d52716bbadae26ef62))

## [0.21.0](https://github.com/andymai/hush/compare/v0.20.0...v0.21.0) (2026-11-10)


### Features

* **history:** suggest terms from corrections ([#288](https://github.com/andymai/hush/issues/288)) ([506a2a0](https://github.com/andymai/hush/commit/506a2a01999372fe6203657931a75ad1c5e5e7e5))

## [0.20.0](https://github.com/andymai/hush/compare/v0.19.0...v0.20.0) (2026-11-10)


### Features

* **audio:** transcribe hands-free takes in parts ([#286](https://github.com/andymai/hush/issues/286)) ([80684d4](https://github.com/andymai/hush/commit/80684d45a24c24492313ecba102dd04b48f42908))

## [0.19.0](https://github.com/andymai/hush/compare/v0.18.0...v0.19.0) (2026-11-10)


### Features

* **history:** keep the audio of each take ([#284](https://github.com/andymai/hush/issues/284)) ([007e3bc](https://github.com/andymai/hush/commit/007e3bcbbd4b79727975d36efc34f16f6ec706c4))

## [0.18.0](https://github.com/andymai/hush/compare/v0.17.0...v0.18.0) (2026-11-10)


### Features

* **transcription:** choose the language for each hotkey and from the tray ([#282](https://github.com/andymai/hush/issues/282)) ([c582218](https://github.com/andymai/hush/commit/c582218238d6250df87284c0fa995d3c77cd3f78))

## [0.17.0](https://github.com/andymai/hush/compare/v0.16.0...v0.17.0) (2026-11-10)


### Features

* **release:** attest the build provenance of each artifact ([#277](https://github.com/andymai/hush/issues/277)) ([026894b](https://github.com/andymai/hush/commit/026894bb3efc87b6e7df8fcbd5b05bc2f74ac53e)), closes [#245](https://github.com/andymai/hush/issues/245)

## [0.16.0](https://github.com/andymai/hush/compare/v0.15.0...v0.16.0) (2026-11-09)


### Features

* **config:** import and export settings and vocabulary ([#273](https://github.com/andymai/hush/issues/273)) ([321a17b](https://github.com/andymai/hush/commit/321a17b7dfeb492511ed6a2cd364e477a4d641b5))
* **transcription:** wait for the GPU at boot and fall back to the CPU ([#272](https://github.com/andymai/hush/issues/272)) ([fdb4343](https://github.com/andymai/hush/commit/fdb43431fb971b350dd15f34ffa92d1d406171ea))


### Bug Fixes

* **audio:** detect a stalled microphone and open it again ([#271](https://github.com/andymai/hush/issues/271)) ([32357ab](https://github.com/andymai/hush/commit/32357abcc0a6298ddeecffcd9cdc87c994c54524))

## [0.15.0](https://github.com/andymai/hush/compare/v0.14.0...v0.15.0) (2026-11-09)


### Features

* **hotkey:** warn about shared chords, AltGr, and character keys ([#269](https://github.com/andymai/hush/issues/269)) ([8e6a2b6](https://github.com/andymai/hush/commit/8e6a2b68a358a475e045d5f7156c9254f0f8a365))

## [0.14.0](https://github.com/andymai/hush/compare/v0.13.0...v0.14.0) (2026-11-09)


### Features

* **audio:** pause media players while Hush records ([#267](https://github.com/andymai/hush/issues/267)) ([875432f](https://github.com/andymai/hush/commit/875432f4230a89ea38c5ea77f24b92d90df8170d))
* **insertion:** keep terminal output on one line and add an optional Enter ([#265](https://github.com/andymai/hush/issues/265)) ([714e9bf](https://github.com/andymai/hush/commit/714e9bf7cefebe4e9f0e8e186798298a0c09bb65))


### Bug Fixes

* **logging:** keep the log file writer alive and keep dictated text out of it ([#266](https://github.com/andymai/hush/issues/266)) ([75cc5ec](https://github.com/andymai/hush/commit/75cc5ec817889ca3b65ffa922d2ec74de7a86f27))

## [0.13.0](https://github.com/andymai/hush/compare/v0.12.1...v0.13.0) (2026-11-09)


### Features

* **transcription:** unload the local model when idle ([#262](https://github.com/andymai/hush/issues/262)) ([1d184c7](https://github.com/andymai/hush/commit/1d184c7b2094aa2a5b9879778b5e2430b88b1802))

## [0.12.1](https://github.com/andymai/hush/compare/v0.12.0...v0.12.1) (2026-11-09)


### Bug Fixes

* **notify:** replace status notifications instead of stacking them ([#260](https://github.com/andymai/hush/issues/260)) ([f6d8ec7](https://github.com/andymai/hush/commit/f6d8ec7b2b25b79308308ed0a02db4fa441d28e5)), closes [#231](https://github.com/andymai/hush/issues/231)

## [0.12.0](https://github.com/andymai/hush/compare/v0.11.0...v0.12.0) (2026-11-09)


### Features

* **cli:** add script output to stop and status ([#254](https://github.com/andymai/hush/issues/254)) ([ad331c2](https://github.com/andymai/hush/commit/ad331c2115670198d3029c261aece2bf187ae7cf))

## [0.11.0](https://github.com/andymai/hush/compare/v0.10.0...v0.11.0) (2026-11-09)


### Features

* **transcription:** trim silence with VAD and drop invented text ([#255](https://github.com/andymai/hush/issues/255)) ([e6b66dd](https://github.com/andymai/hush/commit/e6b66dd2d783d5a539e0a39acc94bc10033789cd))

## [0.10.0](https://github.com/andymai/hush/compare/v0.9.1...v0.10.0) (2026-11-09)


### Features

* **hooks:** run a user command on each transcript ([#252](https://github.com/andymai/hush/issues/252)) ([00dffb3](https://github.com/andymai/hush/commit/00dffb3b941a31031c602830190e9425e8601af8))
* **transcription:** add an OpenAI-compatible backend and LLM provider ([#253](https://github.com/andymai/hush/issues/253)) ([6c9b433](https://github.com/andymai/hush/commit/6c9b43395531298f73b6e3d142bdf339666f2ddb))

## [0.9.1](https://github.com/andymai/hush/compare/v0.9.0...v0.9.1) (2026-11-09)


### Bug Fixes

* **audio:** start the recording cue on the first audio frame ([#233](https://github.com/andymai/hush/issues/233)) ([030cc31](https://github.com/andymai/hush/commit/030cc314c77901ee3a9e6d8fd948aa13c85e3c43))
* **insertion:** restore the clipboard safely, pick paste keys per app, and paste on Dvorak ([#250](https://github.com/andymai/hush/issues/250)) ([c665419](https://github.com/andymai/hush/commit/c6654193f22ce8a47134db5058879229dbc0998e))
* **release:** build ggml for an x86-64-v3 floor ([#249](https://github.com/andymai/hush/issues/249)) ([174a3ac](https://github.com/andymai/hush/commit/174a3ac8c45537a7fe18e8b9ff350d4ee7f5596a))
* **text:** guard LLM polish and match vocabulary phrases as whole words ([#222](https://github.com/andymai/hush/issues/222)) ([1f9aab3](https://github.com/andymai/hush/commit/1f9aab3ed4763c6962197f5d3dfb95022b289186))

## [0.9.0](https://github.com/andymai/hush/compare/v0.8.0...v0.9.0) (2026-11-09)


### Features

* **updates:** opt-in check for new releases ([#212](https://github.com/andymai/hush/issues/212)) ([3bc8ed2](https://github.com/andymai/hush/commit/3bc8ed241410e0bac1bcf019a5a1f86714d50caa))

## [0.8.0](https://github.com/andymai/hush/compare/v0.7.3...v0.8.0) (2026-11-09)


### Features

* **audio:** stop a hands-free recording after a pause ([#204](https://github.com/andymai/hush/issues/204)) ([189e584](https://github.com/andymai/hush/commit/189e5845e8f624aa1a2d9d140bb0b6926a089815)), closes [#111](https://github.com/andymai/hush/issues/111)
* **transcription:** opt-in Groq and OpenAI backends ([#207](https://github.com/andymai/hush/issues/207)) ([3f35687](https://github.com/andymai/hush/commit/3f35687083f596e02814789b587737ffea4dc20a)), closes [#112](https://github.com/andymai/hush/issues/112)
* **ui:** history page, vocabulary editing, and per-app insertion rules ([#208](https://github.com/andymai/hush/issues/208)) ([b3e482a](https://github.com/andymai/hush/commit/b3e482a244f6fffd1ea0296ec5cfea2ba791599b)), closes [#110](https://github.com/andymai/hush/issues/110)

## [0.7.3](https://github.com/andymai/hush/compare/v0.7.2...v0.7.3) (2026-10-14)


### Bug Fixes

* **config:** treat an empty model_path as unset ([#178](https://github.com/andymai/hush/issues/178)) ([ae8d520](https://github.com/andymai/hush/commit/ae8d520c9a06271b743964489c4269df57bd61fd))

## [0.7.2](https://github.com/andymai/hush/compare/v0.7.1...v0.7.2) (2026-10-14)


### Bug Fixes

* **transcription:** drop Whisper non-speech tags like [BLANK_AUDIO] ([#175](https://github.com/andymai/hush/issues/175)) ([c42545f](https://github.com/andymai/hush/commit/c42545f3ecc676011ac66eee62c2a3cb5c3b158e))

## [0.7.1](https://github.com/andymai/hush/compare/v0.7.0...v0.7.1) (2026-09-21)


### Bug Fixes

* **gui:** keep the settings window responsive ([#164](https://github.com/andymai/hush/issues/164)) ([a6e1587](https://github.com/andymai/hush/commit/a6e1587dd8d37407dce1bc8b034f4f57323fae03))

## [0.7.0](https://github.com/andymai/hush/compare/v0.6.1...v0.7.0) (2026-09-21)


### Features

* **gui:** redesign the settings window ([#163](https://github.com/andymai/hush/issues/163)) ([5ed510c](https://github.com/andymai/hush/commit/5ed510cb1240a986fa765293d4bc4f9322f69c83))


### Bug Fixes

* **gui:** drop the non-existent turbo model from the setup hint ([#161](https://github.com/andymai/hush/issues/161)) ([08f7df5](https://github.com/andymai/hush/commit/08f7df5ff5977bfbda3214b5641d29b8dc6f6e73))

## [0.6.1](https://github.com/andymai/hush/compare/v0.6.0...v0.6.1) (2026-09-14)


### Bug Fixes

* **deps:** bump rustls to 0.23.45 for RUSTSEC-2026-0285 ([#159](https://github.com/andymai/hush/issues/159)) ([6e9891d](https://github.com/andymai/hush/commit/6e9891df6f15055272c0ba7009505e134433ac22))

## [0.6.0](https://github.com/andymai/hush/compare/v0.5.0...v0.6.0) (2026-09-04)


### Features

* **cli:** hush doctor ([#152](https://github.com/andymai/hush/issues/152)) ([8fa17db](https://github.com/andymai/hush/commit/8fa17db8f03b6d78172d818ec956d88cc85f2800))
* language detection, layout-aware insertion, and a suggested model ([#153](https://github.com/andymai/hush/issues/153)) ([d65335c](https://github.com/andymai/hush/commit/d65335cc870609e81ba82666e632323b42b9bf8d))
* **ui:** setup and settings window ([#151](https://github.com/andymai/hush/issues/151)) ([3015468](https://github.com/andymai/hush/commit/301546871e782a9193bcefcc196702d2d15d5242))
* **ui:** status icon on the desktop panel ([#149](https://github.com/andymai/hush/issues/149)) ([889220d](https://github.com/andymai/hush/commit/889220d99e5472b6e26d8d9d480356e055cac360))


### Bug Fixes

* **install:** Arch installs the tarball, not a package that does not exist ([#156](https://github.com/andymai/hush/issues/156)) ([67e23ef](https://github.com/andymai/hush/commit/67e23ef735d87a4332c330b64994a6f99987f088))

## [0.5.0](https://github.com/andymai/hush/compare/v0.4.0...v0.5.0) (2026-09-04)


### Features

* Command Mode and an Ollama provider ([#147](https://github.com/andymai/hush/issues/147)) ([4ccc15e](https://github.com/andymai/hush/commit/4ccc15e78ade7492a48a1f85917224127aa26dfa))
* **hotkey:** double-tap to lock, Esc to cancel, and a recording cap ([#144](https://github.com/andymai/hush/issues/144)) ([03c7d7c](https://github.com/andymai/hush/commit/03c7d7c19edb1698216557abc002fce7e3805e09))
* **hotkey:** mouse button hotkeys and an exclusive grab mode ([#141](https://github.com/andymai/hush/issues/141)) ([45bb29f](https://github.com/andymai/hush/commit/45bb29fa6ffac4203277ef343340628ab7701ef2))
* **hotkey:** RightAlt is the default hotkey ([#148](https://github.com/andymai/hush/issues/148)) ([924ae3b](https://github.com/andymai/hush/commit/924ae3b52874fb51543b9cf962af70db3538fc24))
* paste-last and learn commands ([#145](https://github.com/andymai/hush/issues/145)) ([e487147](https://github.com/andymai/hush/commit/e48714737e7bfa8898296d679e092fba2e67f97e))
* tone by app, KWin window detection, and a Whisper context prompt ([#146](https://github.com/andymai/hush/issues/146)) ([d9c38e0](https://github.com/andymai/hush/commit/d9c38e0d3f5ccc4b1a0e7f76a56559d24bfa3488))

## [0.4.0](https://github.com/andymai/hush/compare/v0.3.0...v0.4.0) (2026-09-04)


### Features

* **hotkey:** allow bare modifiers and F13 to F24 as the hotkey ([#139](https://github.com/andymai/hush/issues/139)) ([8692a8c](https://github.com/andymai/hush/commit/8692a8c14600b733fbd080b4289aa4ff384004e9))

## [0.3.0](https://github.com/andymai/hush/compare/v0.2.0...v0.3.0) (2026-09-04)


### Features

* **packaging:** publish releases to AUR and COPR ([#137](https://github.com/andymai/hush/issues/137)) ([7f7635a](https://github.com/andymai/hush/commit/7f7635aeccce3458d435ea2d12c7d95e6c47a1e8))

## [0.2.0](https://github.com/andymai/hush/compare/v0.1.0...v0.2.0) (2026-09-03)


### Features

* Add `hush models set` command for easy model configuration ([98a5d5c](https://github.com/andymai/hush/commit/98a5d5c97c66f064655d383e5bd6f406c797c9c9))
* Add AI Coding Agent Protocol structure ([54929af](https://github.com/andymai/hush/commit/54929afed7b77f4e093a039c463e362f81c82264))
* Add intelligent auto-editing with Wispr Flow-style overlay ([52f1f19](https://github.com/andymai/hush/commit/52f1f19e0382df446bb2413a9130b46b176d2958))
* Add real Claude API integration for text polishing ([0a3a62b](https://github.com/andymai/hush/commit/0a3a62b8f35785da278075eb33ec1a96b0cb6156))
* Add real-time audio amplitude monitoring for waveform ([5b9f296](https://github.com/andymai/hush/commit/5b9f29671c1caa30b9217e0175d19341766e587f))
* Add setup init command for interactive configuration ([66d97a9](https://github.com/andymai/hush/commit/66d97a96870ed7445b24f3e43144921e09abc758))
* **build:** Add platform-aware build system for macOS support ([df36b5a](https://github.com/andymai/hush/commit/df36b5a2b35f7c6e44bbb1c60f8f1604a025a154))
* **config:** Add platform-specific default hotkeys and macOS threading docs ([a990f1c](https://github.com/andymai/hush/commit/a990f1cbbd31f79b1f35dfb28dfdcbf53a2c571a))
* **config:** resolve configuration from XDG paths with compiled defaults ([#117](https://github.com/andymai/hush/issues/117)) ([802fa94](https://github.com/andymai/hush/commit/802fa941c702296e53afdb44fea8e42d087c82b0))
* **daemon:** run the session as a daemon with a control socket ([#124](https://github.com/andymai/hush/issues/124)) ([f6d1b31](https://github.com/andymai/hush/commit/f6d1b31f691a317d56974b7346979cb7e7b15c33))
* **deps:** Add platform-conditional dependencies for macOS support ([e292e62](https://github.com/andymai/hush/commit/e292e62d39c8c8a960e6a8db990113736224acc9))
* Fix overlay positioning and add voice-responsive waveform ([c0d73fb](https://github.com/andymai/hush/commit/c0d73fb8fe22920e747df9affef7bc34020b3e8c))
* **gpu:** Add Metal GPU acceleration support for macOS ([2d6ecd8](https://github.com/andymai/hush/commit/2d6ecd8ba09352b74278043d3fea2bf706596f06))
* **hotkey:** Add macOS main thread detection and enforcement ([ee96893](https://github.com/andymai/hush/commit/ee96893fe1b2666d8ba8dc77813b152bfe03d3bc))
* **hotkey:** read hotkeys from evdev with hold and toggle modes ([#120](https://github.com/andymai/hush/issues/120)) ([75ebfb8](https://github.com/andymai/hush/commit/75ebfb8a709694ee521a479fff8d68eb27937c13))
* Implement missing TODOs and comprehensive documentation audit ([c7df40d](https://github.com/andymai/hush/commit/c7df40df46067e094dcc64773d2ba25965967685))
* Implement Phase 1 & 2 intelligent text processing enhancements ([46f5b63](https://github.com/andymai/hush/commit/46f5b63ff57d94cc6f0ef6cc2a38b3c8e724e176))
* Implement polish & distribution features (Option C) ([3fa3798](https://github.com/andymai/hush/commit/3fa379835ba41e8a37b14b07b292f6392f56b5d2))
* Implement polish & distribution features (Option C) ([baef8b0](https://github.com/andymai/hush/commit/baef8b0e70259fdd7583d88e5c52070275ef32ac))
* Integrate intelligent editing into main CLI application ([c41a60f](https://github.com/andymai/hush/commit/c41a60ff896fe8dac891c8a214b4e92244bf26d3))
* Integrate tiny overlay into main listen command ([ac1ad77](https://github.com/andymai/hush/commit/ac1ad77bd4db959f8f74a28c36e2bb40e1882a52))
* Major codebase improvements - CI/CD, documentation, and refactoring ([6f083ca](https://github.com/andymai/hush/commit/6f083ca04fc7d9a3c7367df978990fa87415cd6c))
* Make CUDA default for release builds, add CPU-only option ([4b64a7c](https://github.com/andymai/hush/commit/4b64a7c0c97051eef2f4156d420d48d053547198))
* Make overlay draggable and always-on-top ([8054d3b](https://github.com/andymai/hush/commit/8054d3b54929172b224e7ad23921cbf9b33fbccc))
* Make overlay tiny with dynamic expansion on recording ([fb3eb62](https://github.com/andymai/hush/commit/fb3eb62802295f706e9359a02affcfd592903ca2))
* **packaging:** build deb, rpm, tarball, and AppImage on release ([#127](https://github.com/andymai/hush/issues/127)) ([b3e27e2](https://github.com/andymai/hush/commit/b3e27e2d3188d56088600910bcd5abbb14ba50e8))
* Redesign overlay to minimal Wispr Flow style ([eab45ae](https://github.com/andymai/hush/commit/eab45ae547bb1b502cbce01d266510f87b60c58a))
* **setup:** grant device access through a udev rule installed with pkexec ([#121](https://github.com/andymai/hush/issues/121)) ([c0960bd](https://github.com/andymai/hush/commit/c0960bd72d5b75308ee0f06b67940aa59fb33cf7))
* **tests:** Add macOS integration tests and permission checking ([72f1631](https://github.com/andymai/hush/commit/72f163146ea3f1ad9e9637a49c4490cf4d4fb0d4))
* **text:** Add macOS text insertion adapter with CGEvent API ([2529ab9](https://github.com/andymai/hush/commit/2529ab92be8ddb02108bec06d6764437d8b61cd7))
* **transcription:** add the Vulkan backend as the default GPU build ([#119](https://github.com/andymai/hush/issues/119)) ([dc9f196](https://github.com/andymai/hush/commit/dc9f196acbbd97341c42a1949a5bccaa37222c3f))
* **tray:** Add macOS system tray adapter using tray-icon ([ae344a9](https://github.com/andymai/hush/commit/ae344a9efd89d230d2b7324db5fc2b72c916c408))
* Update overlay to pill shape with black/white theme ([c0a3f78](https://github.com/andymai/hush/commit/c0a3f784b0a20045cd158c00a32b0f2f10f86573))


### Bug Fixes

* Add proper validation for editing mode parameter ([6319cfe](https://github.com/andymai/hush/commit/6319cfe0fa001b88ca0daef4777d14933c8a0e06))
* Address architecture review issues ([db9ba79](https://github.com/andymai/hush/commit/db9ba798e59b0b97048823cbb947cdd12cd8f2f5))
* Address code review findings - safety, performance, quality ([c177263](https://github.com/andymai/hush/commit/c177263de310246332bcea2c3dd2718dd6703079))
* Address code review issues - performance and safety improvements ([fd54e16](https://github.com/andymai/hush/commit/fd54e16da7ac05f347dad06c83a758e81896a387))
* Change default editing mode to light ([e7465a5](https://github.com/andymai/hush/commit/e7465a51837e7fd2ae9b8938232229f995c846c1))
* CI failures - dependencies and formatting ([e266bf2](https://github.com/andymai/hush/commit/e266bf273ee57661cb9b35993d8196cd28739890))
* Correct expected_size values for Whisper safetensors models ([376469e](https://github.com/andymai/hush/commit/376469e5cb1849faa0df8d4a874cb776f83139bd))
* CUDA build linker issue with empty libcuda.so stub ([b8dfcfa](https://github.com/andymai/hush/commit/b8dfcfa00db08c15bcc4b559bc1e4981651f249b))
* **deps:** adapt to cpal 0.17 DeviceDescription ([9e40a35](https://github.com/andymai/hush/commit/9e40a35436abb2febfb97e7ba0278ba0ee99fcb8))
* **deps:** align audioadapter-buffers with rubato 1.0 ([e5b7167](https://github.com/andymai/hush/commit/e5b7167966a5f8a0caf1a6ccb619fe125eaa59a7))
* **deps:** restore rodio playback feature ([17f33f7](https://github.com/andymai/hush/commit/17f33f7c4d314817677dd68306352a74da7e0163))
* **deps:** update glfw error callback for egui_overlay 0.9 ([616b77c](https://github.com/andymai/hush/commit/616b77ce077e985f8cb6f74d6bd245153e171876))
* Fix failing unit tests ([02d6899](https://github.com/andymai/hush/commit/02d68997ab36bfa1b2c91bdbac646c7a8feb840e))
* Memory leaks causing GPU resource exhaustion during long sessions ([50b2749](https://github.com/andymai/hush/commit/50b274980c85ed72a5896e2162d37ec0a75afbd8))
* Memory leaks causing GPU resource exhaustion during long sessions ([3c074a2](https://github.com/andymai/hush/commit/3c074a277e6a41460e4a2ee0cef41a49a73578cd))
* models list now correctly detects downloaded models ([35d74db](https://github.com/andymai/hush/commit/35d74db184c0548ebf8fef737e24c550d48bf643))
* Prevent overlay focus stealing and fix model path resolution ([6a12036](https://github.com/andymai/hush/commit/6a12036f5aa4065601f45bd38a72693a4d67747a))
* Reduce overlay size to match Wispr Flow proportions ([621bc82](https://github.com/andymai/hush/commit/621bc82710523445472c0e2a9d6d4a03a6f7f1c9))
* Relax CI strictness for initial setup ([2b37a91](https://github.com/andymai/hush/commit/2b37a91ea1b908ec49cbed2dbe6fe3b72e578324))
* repair build after dependency updates, add CI ([13539bf](https://github.com/andymai/hush/commit/13539bf1e36c0e81970c117db87de26db5a22176))
* Replace backreference regex with explicit patterns ([044c7d2](https://github.com/andymai/hush/commit/044c7d2ac9ec992284ed12cf62ab9d96443ba9fa))
* resolve build failures and update dependencies ([a3192df](https://github.com/andymai/hush/commit/a3192dfd0671ea0777525cb986af64555743efd6))
* Resolve compilation errors in test binaries and library tests ([fa6363a](https://github.com/andymai/hush/commit/fa6363af75edb1212982312f2151dcace2905911))
* Resolve TODO items across codebase ([787f58b](https://github.com/andymai/hush/commit/787f58bd6f216fb45d1593bfc14c8fc22d595418))
* Send escape before text insertion to cancel menu focus ([dd53b1e](https://github.com/andymai/hush/commit/dd53b1eb8e4bca9565c2155ecb69031e0f53613b))
* Set default binary to 'hush' for cargo run ([b903702](https://github.com/andymai/hush/commit/b903702eb96de302512da11060dd7478c46e7413))
* Update SHA256 hashes for large Whisper models ([c419d68](https://github.com/andymai/hush/commit/c419d682a4bc25d4ed5cdcdb3232933d79ca1471))
* Use configured audio device in listen mode ([bb053e0](https://github.com/andymai/hush/commit/bb053e021370b534eace93f6ac0013fa3aa93d83))


### Performance

* Implement high-impact performance optimizations ([67506bb](https://github.com/andymai/hush/commit/67506bbadc546d57f50c7f90aa9c31c0b33a1244))
* Implement medium/low-impact optimizations ([c024a3f](https://github.com/andymai/hush/commit/c024a3f8cfa41da198c01aacf4ac9bd78448aa08))
* Improve concurrency - parking_lot RwLock and AtomicBool ([935fe1b](https://github.com/andymai/hush/commit/935fe1b271613eb9a3eae861a2770c4cae8bf784))
* Optimize release build for faster compilation ([202d500](https://github.com/andymai/hush/commit/202d5006137c5dcac43061e6e0e7b3e88a0904b6))
* Reduce hotkey-to-recording latency from ~5s to ~70ms ([16afc29](https://github.com/andymai/hush/commit/16afc292361be80aa511a504701ba484ff99cc20))
