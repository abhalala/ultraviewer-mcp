# Upstream notice

The protocol core and its fixtures were extracted from DeanOpen/MacViewer's `LinuxViewer/` Rust port at commit `f3b56758a0746c74fdacfee006578a9be75c66b3`. The original MacViewer source is MIT-licensed; see `LICENSE` and retain its copyright and permission notice.

The upstream project records REA and Ghidra as analysis tools. Neither tool is distributed here. No UltraViewer Windows executable or library is included.

## UltraViewer screen dictionary

UltraViewer's screen protocol uses a Zstandard decoding dictionary, `8bit.dat`, that UltraViewer supplies: 112,640 bytes, SHA-256 `9aa45789d9ff9d329a623a685d57b67e33c777019de9ce1f273f21860e4bfe6b`, dictionary ID `0x3da0bd10`. It is not independently authored and is not covered by this repository's MIT license. No redistribution license for it has been established.

This repository and its builds do not contain that file. The binary loads a user-supplied copy at runtime and checks it against the size and hash above. See the README section "The screen dictionary".

Rust dependencies keep their own licenses. Zstandard and the WebRTC G.722 codec are used through Rust crates rather than copied into this source tree.
