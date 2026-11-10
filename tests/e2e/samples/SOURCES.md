# Sample sources

Each sample is a 16 kHz mono 16-bit PCM WAV file, the format that Whisper reads.

## jfk.wav

- Source: `samples/jfk.wav` in whisper.cpp,
  <https://github.com/ggml-org/whisper.cpp/blob/master/samples/jfk.wav>.
  Commit `b0a11594aec50892a02cd8d129eee2dfe93a8bb8` is the last commit that
  changed the file.
- Speech: President John F. Kennedy, inaugural address, 20 January 1961.
  "And so, my fellow Americans, ask not what your country can do for you,
  ask what you can do for your country."
- License: public domain. The speech is a work of the US government
  (17 U.S.C. 105).
- Trim: none. The file is the same as in whisper.cpp: 11.0 s, SHA-256
  `59dfb9a4acb36fe2a2affc14bacbee2920ff435cb13cc314a08c13f66ba7860e`.

## verwandlung_de.wav

- Source: the LibriVox recording of "Die Verwandlung" by Franz Kafka, read in
  German by Markus Wachenheim, chapter 1:
  <https://archive.org/details/verwandlung_mw_0811_librivox>, file
  `verwandlung_01_kafka_64kb.mp3` (SHA-1
  `9266657736f83abaaaace4e6f939f4d1ed6638ac`).
- Speech: "Als Gregor Samsa eines Morgens aus unruhigen Träumen erwachte, fand
  er sich in seinem Bett zu einem ungeheueren Ungeziefer verwandelt."
- License: public domain. LibriVox puts its recordings in the public domain,
  and archive.org marks the item with
  <http://creativecommons.org/licenses/publicdomain/>. Kafka published the
  text in 1915, and it is in the public domain.
- Trim: 27.9 s to 37.4 s of the MP3 (9.5 s), SHA-256
  `7502a562f18c4be9572a922f6cccb136a51094c851d02dc9d7f8661b40c6437f`. The
  command:

  ```sh
  ffmpeg -ss 27.9 -to 37.4 -i verwandlung_01_kafka_64kb.mp3 -ac 1 -ar 16000 \
    -c:a pcm_s16le -map_metadata -1 -fflags +bitexact -flags:a +bitexact \
    verwandlung_de.wav
  ```
