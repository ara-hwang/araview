# Third-Party Licenses (AraView)

AraView redistributes the third-party software listed below. Full license
texts ship with the app (Settings > Open source licenses, and the licenses/
folder of the installer).
Generated 2026-09-30. Sections 2 and 3 and THIRD_PARTY_LICENSES.json come from
`node scripts/generate-license-data.mjs` (cargo-about for Rust, node_modules for
npm). Section 1 is maintained by hand from the vcpkg share directory.
Regenerate after dependency changes. CI runs cargo-deny (src-tauri/deny.toml)
to block licenses outside the allow list.

This file is informational, not legal advice.

## 0. Resolved: GPL RAR backend replaced (2026-09-15)

Previous revisions of this file flagged GPL-3.0-or-later code statically
linked via RAR support (unrar-rs plus reedsolomon-rs). The RAR backend is
now rars (MIT OR Apache-2.0, pure Rust, RAR 1.3 through RAR 7). Verified:
unrar-rs and reedsolomon-rs no longer appear in the dependency graph, and
cargo-deny (licenses check, 2026-09-30) reports no GPL/AGPL/LGPL-licensed
crates in the Windows x64 dependency closure. The PSD thumbnail handler
(araview_thumb.dll) closure was permissive all along.

Weak-copyleft note: MPL-2.0 crates (cssparser, cssparser-macros,
dtoa-short, option-ext, selectors) ship via the Tauri stack
(tauri-utils html parsing). MPL-2.0 is file-level: keep the exact
versions listed below so recipients can obtain the unmodified
crates.io sources. Pre-existing, unrelated to the RAR change.

## 1. Dynamically linked native libraries (shipped as separate DLLs)

| Library | License (upstream) | Shipped files | Note |
| ------- | ------------------ | ------------- | ---- |
| libheif (vcpkg libheif[core,aom]) | GNU LGPL | heif.dll | stays dynamically linked, never static |
| libde265 | GNU LGPL | libde265.dll | HEVC decode only, x265 excluded |
| aom | BSD-2-Clause + AOM patent license | aom.dll | AV1 decode for AVIF thumbnails/histogram |

## 2. Rust dependencies (Windows x64 closure from cargo-about)

Includes build-time-only crates (e.g. MPL-2.0 cssparser family via
tauri-build, never shipped in the binary) alongside shipped code. Dev-only
dependencies and other-OS targets are excluded.

| Crate | Version | License |
| ----- | ------- | ------- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| aes | 0.8.4 | MIT OR Apache-2.0 |
| aes | 0.9.3 | MIT OR Apache-2.0 |
| aho-corasick | 1.1.5 | Unlicense OR MIT |
| aligned | 0.4.3 | MIT OR Apache-2.0 |
| aligned-vec | 0.6.4 | MIT |
| alloc-no-stdlib | 2.0.4 | BSD-3-Clause |
| alloc-stdlib | 0.2.4 | BSD-3-Clause |
| anstream | 1.0.0 | MIT OR Apache-2.0 |
| anstyle | 1.0.14 | MIT OR Apache-2.0 |
| anstyle-parse | 1.0.0 | MIT OR Apache-2.0 |
| anstyle-query | 1.1.5 | MIT OR Apache-2.0 |
| anstyle-wincon | 3.0.11 | MIT OR Apache-2.0 |
| anyhow | 1.0.104 | MIT OR Apache-2.0 |
| arg_enum_proc_macro | 0.3.4 | MIT |
| arrayref | 0.3.9 | BSD-2-Clause |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 |
| as-slice | 0.2.1 | MIT OR Apache-2.0 |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT |
| autocfg | 1.5.1 | Apache-2.0 OR MIT |
| av-scenechange | 0.14.1 | MIT |
| av1-grain | 0.2.5 | BSD-2-Clause |
| avif-serialize | 0.8.9 | BSD-3-Clause |
| base64 | 0.22.1 | MIT OR Apache-2.0 |
| base64 | 0.23.1 | MIT OR Apache-2.0 |
| bit_field | 0.10.3 | Apache-2.0 OR MIT |
| bit-set | 0.8.0 | Apache-2.0 OR MIT |
| bit-vec | 0.8.0 | Apache-2.0 OR MIT |
| bitflags | 1.3.2 | MIT OR Apache-2.0 |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| bitstream-io | 4.10.0 | MIT OR Apache-2.0 |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 |
| block-padding | 0.4.2 | MIT OR Apache-2.0 |
| brotli | 8.0.4 | BSD-3-Clause AND MIT |
| brotli-decompressor | 5.0.3 | BSD-3-Clause OR MIT |
| built | 0.8.1 | MIT |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT |
| bytemuck_derive | 1.12.1 | Zlib OR Apache-2.0 OR MIT |
| byteorder | 1.5.0 | Unlicense OR MIT |
| byteorder-lite | 0.1.0 | Unlicense OR MIT |
| bytes | 1.12.1 | MIT |
| bzip2 | 0.5.2 | MIT OR Apache-2.0 |
| bzip2 | 0.6.1 | MIT OR Apache-2.0 |
| bzip2-sys | 0.1.13+1.0.8 | MIT OR Apache-2.0 |
| camino | 1.2.6 | MIT OR Apache-2.0 |
| cargo_metadata | 0.19.2 | MIT |
| cargo_toml | 0.22.3 | Apache-2.0 OR MIT |
| cargo-platform | 0.1.9 | MIT OR Apache-2.0 |
| cbc | 0.2.1 | MIT OR Apache-2.0 |
| cc | 1.4.7 | MIT OR Apache-2.0 |
| cfb | 0.7.3 | MIT |
| cfg-expr | 0.20.9 | MIT OR Apache-2.0 |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 |
| cipher | 0.4.4 | MIT OR Apache-2.0 |
| cipher | 0.5.2 | MIT OR Apache-2.0 |
| cmov | 0.5.4 | Apache-2.0 OR MIT |
| color_quant | 1.1.0 | MIT |
| colorchoice | 1.0.5 | MIT OR Apache-2.0 |
| const-oid | 0.10.2 | Apache-2.0 OR MIT |
| constant_time_eq | 0.3.1 | CC0-1.0 OR MIT-0 OR Apache-2.0 |
| cookie | 0.18.2 | MIT OR Apache-2.0 |
| cpubits | 0.1.1 | MIT OR Apache-2.0 |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 |
| crc | 3.4.0 | MIT OR Apache-2.0 |
| crc-catalog | 2.5.0 | MIT OR Apache-2.0 |
| crc32fast | 1.5.2 | MIT OR Apache-2.0 |
| crossbeam-channel | 0.5.17 | MIT OR Apache-2.0 |
| crossbeam-deque | 0.8.8 | MIT OR Apache-2.0 |
| crossbeam-epoch | 0.9.21 | MIT OR Apache-2.0 |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 |
| cssparser | 0.36.0 | MPL-2.0 |
| cssparser-macros | 0.6.1 | MPL-2.0 |
| ctor | 0.8.0 | Apache-2.0 OR MIT |
| ctor-proc-macro | 0.0.7 | Apache-2.0 OR MIT |
| ctutils | 0.4.2 | Apache-2.0 OR MIT |
| darling | 0.24.1 | MIT |
| darling_core | 0.24.1 | MIT |
| darling_macro | 0.24.1 | MIT |
| data-encoding | 2.11.1 | MIT |
| data-url | 0.3.2 | MIT OR Apache-2.0 |
| deflate64 | 0.1.12 | MIT |
| deranged | 0.5.8 | MIT OR Apache-2.0 |
| derive_more | 2.1.1 | MIT |
| derive_more-impl | 2.1.1 | MIT |
| digest | 0.10.7 | MIT OR Apache-2.0 |
| digest | 0.11.3 | MIT OR Apache-2.0 |
| dirs | 6.0.0 | MIT OR Apache-2.0 |
| dirs-sys | 0.5.0 | MIT OR Apache-2.0 |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 |
| dom_query | 0.27.0 | MIT |
| dpi | 0.1.2 | Apache-2.0 AND MIT |
| dtoa | 1.0.11 | MIT OR Apache-2.0 |
| dtoa-short | 0.3.5 | MPL-2.0 |
| dunce | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 |
| dyn-clone | 1.0.20 | MIT OR Apache-2.0 |
| either | 1.18.0 | MIT OR Apache-2.0 |
| embed-resource | 3.0.11 | MIT |
| enumn | 0.1.14 | MIT OR Apache-2.0 |
| env_filter | 2.0.0 | MIT OR Apache-2.0 |
| env_logger | 0.11.11 | MIT OR Apache-2.0 |
| equator | 0.4.2 | MIT |
| equator-macro | 0.4.2 | MIT |
| equivalent | 1.0.2 | Apache-2.0 OR MIT |
| erased-serde | 0.4.10 | MIT OR Apache-2.0 |
| exr | 1.74.2 | BSD-3-Clause |
| fastrand | 2.5.0 | Apache-2.0 OR MIT |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 |
| filetime | 0.2.29 | MIT OR Apache-2.0 |
| find-msvc-tools | 0.1.13 | MIT OR Apache-2.0 |
| flate2 | 1.1.10 | MIT OR Apache-2.0 |
| float-cmp | 0.9.0 | MIT |
| fnv | 1.0.7 | Apache-2.0  OR  MIT |
| foldhash | 0.2.0 | Zlib |
| font-types | 0.12.5 | MIT OR Apache-2.0 |
| fontdb | 0.24.0 | MIT |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 |
| four-cc | 0.4.0 | MIT OR Apache-2.0 |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 |
| futures-core | 0.3.34 | MIT OR Apache-2.0 |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 |
| futures-sink | 0.3.34 | MIT OR Apache-2.0 |
| futures-task | 0.3.34 | MIT OR Apache-2.0 |
| futures-util | 0.3.34 | MIT OR Apache-2.0 |
| generic-array | 0.14.7 | MIT |
| getrandom | 0.2.17 | MIT OR Apache-2.0 |
| getrandom | 0.3.4 | MIT OR Apache-2.0 |
| getrandom | 0.4.3 | MIT OR Apache-2.0 |
| gif | 0.14.2 | MIT OR Apache-2.0 |
| glob | 0.3.4 | MIT OR Apache-2.0 |
| half | 2.7.1 | MIT OR Apache-2.0 |
| harfrust | 0.12.0 | MIT |
| hashbrown | 0.12.3 | MIT OR Apache-2.0 |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| heck | 0.5.0 | MIT OR Apache-2.0 |
| hmac | 0.12.1 | MIT OR Apache-2.0 |
| hmac | 0.13.0 | MIT OR Apache-2.0 |
| html5ever | 0.38.0 | MIT OR Apache-2.0 |
| http | 1.5.0 | MIT OR Apache-2.0 |
| http-body | 1.1.0 | MIT |
| http-body-util | 0.1.5 | MIT |
| http-range | 0.1.5 | MIT |
| httparse | 1.10.1 | MIT OR Apache-2.0 |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 |
| hyper | 1.11.1 | MIT |
| hyper-rustls | 0.27.10 | Apache-2.0 OR ISC OR MIT |
| hyper-util | 0.1.20 | MIT |
| ico | 0.5.0 | MIT |
| icu_collections | 2.3.0 | Unicode-3.0 |
| icu_locale_core | 2.3.0 | Unicode-3.0 |
| icu_normalizer | 2.3.0 | Unicode-3.0 |
| icu_normalizer_data | 2.3.0 | Unicode-3.0 |
| icu_properties | 2.3.0 | Unicode-3.0 |
| icu_properties_data | 2.3.0 | Unicode-3.0 |
| icu_provider | 2.3.1 | Unicode-3.0 |
| ident_case | 1.0.1 | MIT OR Apache-2.0 |
| idna | 1.1.0 | MIT OR Apache-2.0 |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT |
| image | 0.25.10 | MIT OR Apache-2.0 |
| image-webp | 0.2.4 | MIT OR Apache-2.0 |
| imagesize | 0.15.0 | MIT |
| imgref | 1.12.3 | CC0-1.0 OR Apache-2.0 |
| indexmap | 1.9.3 | Apache-2.0 OR MIT |
| indexmap | 2.14.2 | Apache-2.0 OR MIT |
| infer | 0.19.0 | MIT |
| inout | 0.1.4 | MIT OR Apache-2.0 |
| inout | 0.2.2 | MIT OR Apache-2.0 |
| ipnet | 2.12.2 | MIT OR Apache-2.0 |
| is_terminal_polyfill | 1.70.2 | MIT OR Apache-2.0 |
| itertools | 0.14.0 | MIT OR Apache-2.0 |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| jiff | 0.2.37 | Unlicense OR MIT |
| jiff-core | 0.1.1 | Unlicense OR MIT |
| jobserver | 0.1.35 | MIT OR Apache-2.0 |
| jpeg-encoder | 0.7.1 | (MIT OR Apache-2.0) AND IJG |
| json-patch | 3.0.1 | MIT OR Apache-2.0 |
| jsonptr | 0.6.3 | MIT OR Apache-2.0 |
| kamadak-exif | 0.6.1 | BSD-2-Clause |
| keyboard-types | 0.7.0 | MIT OR Apache-2.0 |
| kurbo | 0.13.1 | Apache-2.0 OR MIT |
| lebe | 0.5.3 | BSD-3-Clause |
| libbz2-rs-sys | 0.2.5 | bzip2-1.0.6 |
| libc | 0.2.189 | MIT OR Apache-2.0 |
| libheif-rs | 3.0.0 | MIT |
| libheif-sys | 5.3.1+1.23.1 | MIT |
| libm | 0.2.16 | MIT |
| litemap | 0.8.3 | Unicode-3.0 |
| lock_api | 0.4.14 | MIT OR Apache-2.0 |
| log | 0.4.34 | MIT OR Apache-2.0 |
| loop9 | 0.1.5 | MIT |
| lzma-rs | 0.3.0 | MIT |
| lzma-rust2 | 0.20.1 | Apache-2.0 |
| lzma-sys | 0.1.20 | MIT OR Apache-2.0 |
| markup5ever | 0.38.0 | MIT OR Apache-2.0 |
| maybe-rayon | 0.1.1 | MIT |
| memchr | 2.8.3 | Unlicense OR MIT |
| mime | 0.3.17 | MIT OR Apache-2.0 |
| minisign-verify | 0.2.5 | MIT |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| mio | 1.2.3 | MIT |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 |
| muda | 0.19.3 | Apache-2.0 OR MIT |
| mutate_once | 0.1.2 | BSD-2-Clause |
| new_debug_unreachable | 1.0.6 | MIT |
| no_std_io2 | 0.9.4 | Apache-2.0 OR MIT |
| nom | 8.0.0 | MIT |
| noop_proc_macro | 0.3.0 | MIT |
| notify | 8.2.0 | CC0-1.0 |
| notify-types | 2.1.0 | MIT OR Apache-2.0 |
| num-bigint | 0.4.8 | MIT OR Apache-2.0 |
| num-complex | 0.4.6 | MIT OR Apache-2.0 |
| num-conv | 0.2.2 | MIT OR Apache-2.0 |
| num-derive | 0.4.2 | MIT OR Apache-2.0 |
| num-integer | 0.1.47 | MIT OR Apache-2.0 |
| num-rational | 0.4.2 | MIT OR Apache-2.0 |
| num-traits | 0.2.19 | MIT OR Apache-2.0 |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| once_cell_polyfill | 1.70.2 | MIT OR Apache-2.0 |
| open | 5.4.4 | MIT |
| option-ext | 0.2.0 | MPL-2.0 |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 |
| paste | 1.0.15 | MIT OR Apache-2.0 |
| pastey | 0.1.1 | MIT OR Apache-2.0 |
| pbkdf2 | 0.12.2 | MIT OR Apache-2.0 |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 |
| phf | 0.13.1 | MIT |
| phf_codegen | 0.13.1 | MIT |
| phf_generator | 0.13.1 | MIT |
| phf_macros | 0.13.1 | MIT |
| phf_shared | 0.13.1 | MIT |
| pico-args | 0.5.0 | MIT |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 |
| plist | 1.10.1 | MIT |
| png | 0.17.16 | MIT OR Apache-2.0 |
| png | 0.18.1 | MIT OR Apache-2.0 |
| polycool | 0.4.0 | MIT OR Apache-2.0 |
| potential_utf | 0.1.6 | Unicode-3.0 |
| powerfmt | 0.2.0 | MIT OR Apache-2.0 |
| ppmd-rust | 1.5.0 | CC0-1.0 OR MIT-0 |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 |
| precomputed-hash | 0.1.1 | MIT |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| profiling | 1.0.18 | MIT OR Apache-2.0 |
| profiling-procmacros | 1.0.18 | MIT OR Apache-2.0 |
| psd | 0.3.5 | MIT OR Apache-2.0 |
| pulp | 0.22.3 | MIT |
| pulp-wasm-simd-flag | 0.1.1 | MIT |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 |
| qoi | 0.4.1 | MIT OR Apache-2.0 |
| quick-error | 2.0.1 | MIT OR Apache-2.0 |
| quick-xml | 0.42.0 | MIT |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| rand | 0.9.5 | MIT OR Apache-2.0 |
| rand_chacha | 0.9.0 | MIT OR Apache-2.0 |
| rand_core | 0.9.5 | MIT OR Apache-2.0 |
| rars | 0.9.4 | MIT OR Apache-2.0 |
| rav1e | 0.8.1 | BSD-2-Clause |
| ravif | 0.13.0 | BSD-3-Clause |
| raw-cpuid | 11.6.0 | MIT |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib |
| rayon | 1.12.0 | MIT OR Apache-2.0 |
| rayon-core | 1.13.0 | MIT OR Apache-2.0 |
| read-fonts | 0.41.0 | MIT OR Apache-2.0 |
| reborrow | 0.5.5 | MIT |
| regex | 1.13.1 | MIT OR Apache-2.0 |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 |
| reqwest | 0.13.5 | MIT OR Apache-2.0 |
| resvg | 0.48.1 | Apache-2.0 OR MIT |
| rfd | 0.16.0 | MIT |
| rgb | 0.8.53 | MIT |
| ring | 0.17.14 | Apache-2.0 AND ISC |
| roxmltree | 0.21.1 | MIT OR Apache-2.0 |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT |
| rustls | 0.23.45 | Apache-2.0 OR ISC OR MIT |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 |
| rustls-platform-verifier | 0.7.0 | MIT OR Apache-2.0 |
| rustls-webpki | 0.103.15 | ISC |
| same-file | 1.0.6 | Unlicense OR MIT |
| schemars | 0.8.22 | MIT |
| schemars_derive | 0.8.22 | MIT |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 |
| selectors | 0.36.1 | MPL-2.0 |
| semver | 1.0.28 | MIT OR Apache-2.0 |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive_internals | 0.29.1 | MIT OR Apache-2.0 |
| serde_json | 1.0.151 | MIT OR Apache-2.0 |
| serde_repr | 0.1.21 | MIT OR Apache-2.0 |
| serde_spanned | 1.1.1 | MIT OR Apache-2.0 |
| serde_with | 3.23.0 | MIT OR Apache-2.0 |
| serde_with_macros | 3.23.0 | MIT OR Apache-2.0 |
| serde-untagged | 0.1.9 | MIT OR Apache-2.0 |
| serialize-to-javascript | 0.1.2 | MIT OR Apache-2.0 |
| serialize-to-javascript-impl | 0.1.2 | MIT OR Apache-2.0 |
| servo_arc | 0.4.3 | MIT OR Apache-2.0 |
| sevenz-rust2 | 0.22.2 | Apache-2.0 |
| sha1 | 0.10.7 | MIT OR Apache-2.0 |
| sha1 | 0.11.0 | MIT OR Apache-2.0 |
| sha2 | 0.10.9 | MIT OR Apache-2.0 |
| sha2 | 0.11.0 | MIT OR Apache-2.0 |
| shlex | 2.0.1 | MIT OR Apache-2.0 |
| simd_helpers | 0.1.0 | MIT |
| simd-adler32 | 0.3.10 | MIT |
| simplecss | 0.2.2 | Apache-2.0 OR MIT |
| siphasher | 1.0.3 | MIT OR Apache-2.0 |
| skrifa | 0.44.0 | MIT OR Apache-2.0 |
| slab | 0.4.12 | MIT |
| slotmap | 1.1.1 | Zlib |
| smallvec | 1.16.1 | MIT OR Apache-2.0 |
| socket2 | 0.6.5 | MIT OR Apache-2.0 |
| softbuffer | 0.4.8 | MIT OR Apache-2.0 |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 |
| strict-num | 0.1.1 | MIT |
| string_cache | 0.9.0 | MIT OR Apache-2.0 |
| string_cache_codegen | 0.6.1 | MIT OR Apache-2.0 |
| strsim | 0.11.1 | MIT |
| subtle | 2.6.1 | BSD-3-Clause |
| svgtypes | 0.16.1 | Apache-2.0 OR MIT |
| syn | 2.0.119 | MIT OR Apache-2.0 |
| syn | 3.0.6 | MIT OR Apache-2.0 |
| sync_wrapper | 1.0.2 | Apache-2.0 |
| synstructure | 0.14.0 | MIT |
| system-deps | 8.0.0 | MIT OR Apache-2.0 |
| tao | 0.35.3 | Apache-2.0 |
| tar | 0.4.46 | MIT OR Apache-2.0 |
| target-lexicon | 0.13.5 | Apache-2.0 WITH LLVM-exception |
| tauri | 2.11.6 | Apache-2.0 OR MIT |
| tauri-build | 2.6.3 | Apache-2.0 OR MIT |
| tauri-codegen | 2.6.3 | Apache-2.0 OR MIT |
| tauri-macros | 2.6.3 | Apache-2.0 OR MIT |
| tauri-plugin | 2.6.3 | Apache-2.0 OR MIT |
| tauri-plugin-dialog | 2.7.3 | Apache-2.0 OR MIT |
| tauri-plugin-fs | 2.5.2 | Apache-2.0 OR MIT |
| tauri-plugin-mcp-bridge | 0.13.0 | MIT |
| tauri-plugin-opener | 2.5.5 | Apache-2.0 OR MIT |
| tauri-plugin-single-instance | 2.4.5 | Apache-2.0 OR MIT |
| tauri-plugin-snap-layout | 1.0.9 | MIT |
| tauri-plugin-store | 2.4.5 | Apache-2.0 OR MIT |
| tauri-plugin-updater | 2.12.0 | Apache-2.0 OR MIT |
| tauri-plugin-window-state | 2.4.1 | Apache-2.0 OR MIT |
| tauri-runtime | 2.11.3 | Apache-2.0 OR MIT |
| tauri-runtime-wry | 2.11.4 | Apache-2.0 OR MIT |
| tauri-utils | 2.9.3 | Apache-2.0 OR MIT |
| tauri-winres | 0.3.6 | MIT |
| tempfile | 3.27.0 | MIT OR Apache-2.0 |
| tendril | 0.5.1 | MIT OR Apache-2.0 |
| thiserror | 1.0.69 | MIT OR Apache-2.0 |
| thiserror | 2.0.20 | MIT OR Apache-2.0 |
| thiserror-impl | 1.0.69 | MIT OR Apache-2.0 |
| thiserror-impl | 2.0.20 | MIT OR Apache-2.0 |
| time | 0.3.55 | MIT OR Apache-2.0 |
| time-core | 0.1.9 | MIT OR Apache-2.0 |
| time-macros | 0.2.32 | MIT OR Apache-2.0 |
| tiny-skia | 0.12.0 | BSD-3-Clause |
| tiny-skia-path | 0.12.0 | BSD-3-Clause |
| tinystr | 0.8.4 | Unicode-3.0 |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT |
| tokio | 1.53.1 | MIT |
| tokio-macros | 2.7.2 | MIT |
| tokio-rustls | 0.26.5 | MIT OR Apache-2.0 |
| tokio-tungstenite | 0.28.0 | MIT |
| tokio-util | 0.7.19 | MIT |
| toml | 0.9.12+spec-1.1.0 | MIT OR Apache-2.0 |
| toml | 1.1.6+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_datetime | 0.7.5+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_datetime | 1.1.1+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_parser | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_writer | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 |
| tower | 0.5.3 | MIT |
| tower-http | 0.6.11 | MIT |
| tower-layer | 0.3.3 | MIT |
| tower-service | 0.3.3 | MIT |
| tracing | 0.1.44 | MIT |
| tracing-attributes | 0.1.31 | MIT |
| tracing-core | 0.1.36 | MIT |
| trash | 3.3.1 | MIT |
| try-lock | 0.2.5 | MIT |
| tungstenite | 0.28.0 | MIT OR Apache-2.0 |
| typeid | 1.0.3 | MIT OR Apache-2.0 |
| typenum | 1.20.1 | MIT OR Apache-2.0 |
| unic-char-property | 0.9.0 | MIT OR Apache-2.0 |
| unic-char-range | 0.9.0 | MIT OR Apache-2.0 |
| unic-common | 0.9.0 | MIT OR Apache-2.0 |
| unic-ucd-ident | 0.9.0 | MIT OR Apache-2.0 |
| unic-ucd-version | 0.9.0 | MIT OR Apache-2.0 |
| unicode-bidi | 0.3.18 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.26 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| unicode-script | 0.5.8 | MIT OR Apache-2.0 |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 |
| unicode-vo | 0.1.0 | MIT OR Apache-2.0 |
| untrusted | 0.9.0 | ISC |
| url | 2.5.8 | MIT OR Apache-2.0 |
| urlpattern | 0.3.0 | MIT |
| usvg | 0.48.1 | Apache-2.0 OR MIT |
| utf-8 | 0.7.6 | MIT OR Apache-2.0 |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT |
| utf8parse | 0.2.2 | Apache-2.0 OR MIT |
| uuid | 1.26.1 | Apache-2.0 OR MIT |
| v_frame | 0.3.9 | BSD-2-Clause |
| vcpkg | 0.2.15 | MIT OR Apache-2.0 |
| version_check | 0.9.5 | MIT OR Apache-2.0 |
| version-compare | 0.2.1 | MIT |
| vswhom | 0.1.0 | MIT |
| vswhom-sys | 0.1.3 | MIT |
| walkdir | 2.5.0 | Unlicense OR MIT |
| want | 0.3.1 | MIT |
| web_atoms | 0.2.6 | MIT OR Apache-2.0 |
| webview2-com | 0.38.2 | MIT |
| webview2-com-macros | 0.8.1 | MIT |
| webview2-com-sys | 0.38.2 | MIT |
| weezl | 0.1.12 | MIT OR Apache-2.0 |
| winapi-util | 0.1.11 | Unlicense OR MIT |
| window-vibrancy | 0.6.0 | Apache-2.0 OR MIT |
| windows | 0.44.0 | MIT OR Apache-2.0 |
| windows | 0.61.3 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.42.2 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.53.1 | MIT OR Apache-2.0 |
| windows-collections | 0.2.0 | MIT OR Apache-2.0 |
| windows-core | 0.61.2 | MIT OR Apache-2.0 |
| windows-future | 0.2.1 | MIT OR Apache-2.0 |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 |
| windows-link | 0.1.3 | MIT OR Apache-2.0 |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-numerics | 0.2.0 | MIT OR Apache-2.0 |
| windows-registry | 0.6.1 | MIT OR Apache-2.0 |
| windows-result | 0.3.4 | MIT OR Apache-2.0 |
| windows-result | 0.4.1 | MIT OR Apache-2.0 |
| windows-strings | 0.4.2 | MIT OR Apache-2.0 |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 |
| windows-sys | 0.60.2 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| windows-targets | 0.42.2 | MIT OR Apache-2.0 |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 |
| windows-targets | 0.53.5 | MIT OR Apache-2.0 |
| windows-threading | 0.1.0 | MIT OR Apache-2.0 |
| windows-version | 0.1.7 | MIT OR Apache-2.0 |
| winnow | 0.7.15 | MIT |
| winnow | 1.0.4 | MIT |
| winreg | 0.55.0 | MIT |
| winreg | 0.56.0 | MIT |
| writeable | 0.6.4 | Unicode-3.0 |
| wry | 0.55.1 | Apache-2.0 OR MIT |
| xmlwriter | 0.1.0 | MIT |
| xz2 | 0.1.7 | MIT OR Apache-2.0 |
| y4m | 0.8.0 | MIT |
| yoke | 0.8.3 | Unicode-3.0 |
| yoke-derive | 0.8.3 | Unicode-3.0 |
| zerocopy | 0.8.57 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerocopy-derive | 0.8.57 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerofrom | 0.1.8 | Unicode-3.0 |
| zerofrom-derive | 0.1.8 | Unicode-3.0 |
| zeroize | 1.9.0 | Apache-2.0 OR MIT |
| zeroize_derive | 1.5.0 | Apache-2.0 OR MIT |
| zerotrie | 0.2.5 | Unicode-3.0 |
| zerovec | 0.11.8 | Unicode-3.0 |
| zerovec-derive | 0.11.6 | Unicode-3.0 |
| zip | 2.4.2 | MIT |
| zip | 4.6.1 | MIT |
| zmij | 1.0.23 | MIT |
| zopfli | 0.8.3 | Apache-2.0 |
| zstd | 0.13.3 | MIT |
| zstd-safe | 7.3.0 | BSD-3-Clause |
| zstd-sys | 2.1.0+zstd.1.5.7 | BSD-3-Clause |
| zune-core | 0.5.3 | MIT OR Apache-2.0 OR Zlib |
| zune-inflate | 0.2.54 | MIT OR Apache-2.0 OR Zlib |
| zune-jpeg | 0.5.15 | MIT OR Apache-2.0 OR Zlib |

## 3. npm production dependencies (bundled into dist/)

Pretendard font (OFL-1.1): no license text file found in pretendard npm package.

| Package | Version (installed) | License |
| ------- | ------------------- | ------- |
| @base-ui/react | 1.8.0 | MIT |
| @phosphor-icons/react | 2.1.10 | MIT |
| @tailwindcss/vite | 4.3.3 | MIT |
| @tanstack/react-router | 1.170.38 | MIT |
| @tauri-apps/api | 2.11.1 | Apache-2.0 OR MIT |
| @tauri-apps/plugin-dialog | 2.7.3 | MIT OR Apache-2.0 |
| @tauri-apps/plugin-opener | 2.5.5 | MIT OR Apache-2.0 |
| @tauri-apps/plugin-store | 2.4.5 | MIT OR Apache-2.0 |
| @tauri-apps/plugin-updater | 2.12.0 | MIT OR Apache-2.0 |
| class-variance-authority | 0.7.1 | Apache-2.0 |
| cn | 0.2.6 | MIT |
| i18next | 26.4.2 | MIT |
| pretendard | 1.3.9 | OFL-1.1 |
| react | 19.3.0 | MIT |
| react-dom | 19.3.0 | MIT |
| react-i18next | 17.0.14 | MIT |
| tailwindcss | 4.3.3 | MIT |
| tauri-plugin-snap-layout | 1.0.9 | MIT |
| tw-animate-css | 1.4.0 | MIT |
| zustand | 5.0.15 | MIT |

## 4. Research reference notice (not a bundled dependency)

`src-tauri/src/pixel_art.rs` was informed by the run-length, palette, Sobel
profile, and autocorrelation ideas in [unfake.js](https://github.com/jenissimo/unfake.js),
pinned for reference at commit `b2bee10c1c3b211a2532baca9088857b19480dca`.
AraView contains an independent Rust implementation and does not ship the
upstream JavaScript or add it as a runtime dependency. The upstream MIT notice
is reproduced here for provenance:

> MIT License
>
> Copyright (c) 2024 unfake.js team
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.
| zustand | 5.0.15 | MIT |

