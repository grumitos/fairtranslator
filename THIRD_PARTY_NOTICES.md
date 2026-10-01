FairTranslator Third-Party Notices

Last updated: 2026-09-30

FairTranslator includes third-party software components. Those components remain
subject to their own licenses and copyright notices.

For commercial distributions of FairTranslator:

- include this file with the installer or bundled application
- preserve upstream notices for MIT, Apache-2.0, ISC, MPL-2.0, and OFL-1.1 components
- if you ever modify MPL-2.0-covered files and distribute the resulting build, make the modified MPL-covered source files available under MPL-2.0 as required by that license
- keep the OFL-1.1 copyright notice and license text with any redistributed font files (reproduced at the end of this file)

The frontend sections of this notice file reflect package-lock.json on 2026-09-30.
The Rust sections were last reviewed on 2026-07-10.
Regenerate and review it before each public release.

Frontend runtime and build tooling dependencies

| Component | Version | License | Upstream |
| --- | --- | --- | --- |
| @fontsource-variable/geist | 5.3.0 | OFL-1.1 | https://github.com/vercel/geist-font |
| @fontsource-variable/geist-mono | 5.3.0 | OFL-1.1 | https://github.com/vercel/geist-font |
| @tailwindcss/vite | 4.3.3 | MIT | https://tailwindcss.com |
| @tanstack/react-table | 8.21.3 | MIT | https://tanstack.com/table |
| @tanstack/react-virtual | 3.14.13 | MIT | https://tanstack.com/virtual |
| @tauri-apps/api | 2.10.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri#readme |
| @tauri-apps/plugin-dialog | 2.6.0 | MIT OR Apache-2.0 | https://github.com/tauri-apps/plugins-workspace |
| class-variance-authority | 0.7.1 | Apache-2.0 | https://github.com/joe-bell/cva#readme |
| clsx | 2.1.1 | MIT | https://github.com/lukeed/clsx |
| i18next | 25.10.10 | MIT | https://www.i18next.com |
| immer | 11.1.18 | MIT | https://github.com/immerjs/immer#readme |
| lucide-react | 0.576.0 | ISC | https://lucide.dev |
| radix-ui | 1.6.7 | MIT | https://radix-ui.com/primitives |
| react | 19.3.0 | MIT | https://react.dev/ |
| react-dom | 19.3.0 | MIT | https://react.dev/ |
| react-i18next | 16.6.6 | MIT | https://github.com/i18next/react-i18next |
| sonner | 2.0.8 | MIT | https://sonner.emilkowal.ski/ |
| tailwind-merge | 3.7.0 | MIT | https://github.com/dcastil/tailwind-merge |
| tailwindcss | 4.3.3 | MIT | https://tailwindcss.com |
| zustand | 5.0.15 | MIT | https://github.com/pmndrs/zustand |

Rust application dependencies

| Component | Version | License | Upstream |
| --- | --- | --- | --- |
| async-trait | 0.1.89 | MIT OR Apache-2.0 | https://github.com/dtolnay/async-trait |
| chrono | 0.4.44 | MIT OR Apache-2.0 | https://github.com/chronotope/chrono |
| log | 0.4.29 | MIT OR Apache-2.0 | https://github.com/rust-lang/log |
| regex | 1.12.3 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| reqwest | 0.12.28 | MIT OR Apache-2.0 | https://github.com/seanmonstar/reqwest |
| rpgtool-common | 0.2.0 | MPL-2.0 | https://github.com/melody-rs/rpgtool |
| serde | 1.0.228 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_json | 1.0.149 | MIT OR Apache-2.0 | https://github.com/serde-rs/json |
| tauri | 2.10.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-plugin-dialog | 2.6.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-log | 2.8.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-store | 2.4.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| thiserror | 2.0.18 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| tokio | 1.50.0 | MIT | https://github.com/tokio-rs/tokio |
| uuid | 1.21.0 | Apache-2.0 OR MIT | https://github.com/uuid-rs/uuid |

MPL-2.0 components present in the Rust dependency graph

| Component | Version | License | Upstream |
| --- | --- | --- | --- |
| alox-48 | 0.6.0 | MPL-2.0 | https://github.com/Speak2Erase/alox-48 |
| alox-48-derive | 0.3.3 | MPL-2.0 | https://github.com/Speak2Erase/alox-48 |
| cssparser | 0.29.6 | MPL-2.0 | https://github.com/servo/rust-cssparser |
| cssparser-macros | 0.6.1 | MPL-2.0 | https://github.com/servo/rust-cssparser |
| dtoa-short | 0.3.5 | MPL-2.0 | https://github.com/upsuper/dtoa-short |
| option-ext | 0.2.0 | MPL-2.0 | https://github.com/soc/option-ext.git |
| rpgtool-common | 0.2.0 | MPL-2.0 | https://github.com/melody-rs/rpgtool |
| selectors | 0.24.0 | MPL-2.0 | https://github.com/servo/servo |

Practical compliance note

FairTranslator is not open source, but some dependencies are. Permissive
licenses such as MIT, Apache-2.0, and ISC generally require preserving notices.
MPL-2.0 is a file-level copyleft license: if FairTranslator distributes modified
MPL-covered files, the modified MPL-covered source files must remain available
under MPL-2.0. That obligation does not automatically require releasing the
entire FairTranslator codebase.

The bundled Geist and Geist Mono fonts are licensed under the SIL Open Font
License 1.1 (OFL-1.1). The fonts may be bundled and redistributed with
FairTranslator, but they may not be sold on their own, and each copy must carry
the copyright notice and the license text, which are reproduced in full below.
This file ships with the installer, so it satisfies that condition as long as it
is kept in the bundle.

SIL Open Font License 1.1 (Geist and Geist Mono)

Copyright 2024 The Geist Project Authors (https://github.com/vercel/geist-font)

This Font Software is licensed under the SIL Open Font License, Version 1.1.
This license is copied below, and is also available with a FAQ at:
http://scripts.sil.org/OFL

-----------------------------------------------------------
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded,
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
