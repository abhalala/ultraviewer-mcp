# Third-party notices

## MacViewer

The protocol core and its test fixtures were extracted from
[DeanOpen/MacViewer](https://github.com/DeanOpen/MacViewer), `LinuxViewer/`,
commit `f3b56758a0746c74fdacfee006578a9be75c66b3`, under the MIT license in
`LICENSE`.

REA and Ghidra are analysis tools used by the upstream project and are not
distributed here.

## G.722 decoder

`src/audio.rs` contains a Rust port of the SpanDSP/WebRTC 64 kbit/s G.722
decoder, by way of MacViewer's `ThirdParty/g722` at WebRTC revision
`4fdd50a57b79826ea87cc5170be78b6444d9cdf9`. The original public-domain notice
and the WebRTC BSD license are preserved in `licenses/g722/LICENSE` and
`licenses/g722/WEBRTC_LICENSE`.

## Zstandard

Zstandard decompression is linked through the `zstd-safe` and `zstd-sys`
crates, which bundle Zstandard under its BSD license (with the GPLv2
alternative). Other Rust dependencies keep their own licenses.

## UltraViewer screen dictionary

The repository and the binary include the UltraViewer-supplied
`UltraViewer/8bit.dat` Zstandard decoding dictionary as data at
`assets/8bit.dat`, embedded at build time. It is 112,640 bytes with SHA-256
`9aa45789d9ff9d329a623a685d57b67e33c777019de9ce1f273f21860e4bfe6b` and
dictionary ID `0x3da0bd10`. This is an UltraViewer-supplied asset, not an
independently authored or open-source dictionary, and it is not covered by the
MIT license. No redistribution license for that asset was established; it is
included for interoperability with UltraViewer's screen protocol.

No UltraViewer Windows executable or library code is included.
