# What is copied into `site/`, where from, and under what licence

Everything in `site/` is a copy of a file of [alpibrusl/cancho-www](https://github.com/alpibrusl/cancho-www) at commit `7578537` (read-only reference; nothing was pushed to it), made by `cp`,
byte for byte (the hashes below are the ones the compiler checks). cancho-www and cancho-ui are both EUPL-1.2, Copyright (c) 2026 Alpibru S.L.; the text of the page and its data are the page's own
and carry its licence, which is the licence of this repository.

## Fonts (self-hosted; the page makes no request to a font CDN)

| file | what | source | licence | SHA-256 |
|---|---|---|---|---|
| `fonts/plus-jakarta-sans-latin-wght-normal.woff2` | Plus Jakarta Sans, variable, latin subset (body) | npm `@fontsource-variable/plus-jakarta-sans` 5.3.0, from https://github.com/tokotype/PlusJakartaSans | **SIL OFL 1.1**, `fonts/OFL-PlusJakartaSans.txt` | `153fc85b70298beeb1d61a5f723331649e7f23bb77302a66e61cb3e2fbdb5e79` |
| `fonts/jetbrains-mono-latin-wght-normal.woff2` | JetBrains Mono, variable, latin subset (code) | npm `@fontsource-variable/jetbrains-mono` 5.3.0, from https://github.com/JetBrains/JetBrainsMono | **SIL OFL 1.1**, `fonts/OFL-JetBrainsMono.txt` | `18be452724bfdc236c074ca94a249a7f41a86752c7d04ab258ce9ed5651f6a7e` |
| `fonts/caveat-latin-wght-normal.woff2` | Caveat, variable, latin subset (the hand of the map; declared now, drawn from B2) | npm `@fontsource-variable/caveat` 5.3.0, from https://github.com/googlefonts/caveat | **SIL OFL 1.1**, `fonts/OFL-Caveat.txt` | `891951df5e8b5b0f0bc760d31375405e795c666b5add70437de12d4a6482b33f` |

**Licences checked** (2026-10-26, by reading the three `OFL-*.txt` files copied with the fonts: each begins with its copyright line and "This Font Software is licensed under the SIL Open Font License, Version 1.1"):
the SIL OFL 1.1 is a permissive licence for fonts: the fonts may be used, embedded and redistributed, modified or not, **provided the copyright notice and the licence text travel with them** (they do:
beside the files) and the fonts are not sold on their own; the "Reserved Font Name" clause applies to modified versions only, and these are the unmodified fontsource builds. It is compatible with
shipping them in an EUPL-1.2 repository. The compiler checks each font's hash and that a licence is declared (`font-hash-mismatch`, `font-licence-missing`); `scripts/check-www-port.js` checks that the
OFL text is beside each file. This is a reading of the licence texts by the author of the PR and **not legal advice**.

## Pictures

| file | what | licence |
|---|---|---|
| `assets/img/field.*`, `dawn.*`, `balance.*`, `lichen.*` (webp and jpg) | the four illustrations of the page | generated with Mistral AI and supplied by the project owner; recompressed, not otherwise altered. **Not a permissive licence**: the owner is responsible for the generator's terms permitting this use (cancho-www's `THIRD_PARTY.md` says the same). The page says on its face that they are generated. |
| `og.png` | the card image of the link preview | made by the page's `scripts/make_og.js` from `scripts/og/stones.jpg` (the same generated picture) |
| `logo.png`, `logo-mark.png` | the logo | the project's |
| `assets/media/ui-demo.*`, `demo/demo.*`, `demo/poster.png`, `demo/demo.cast`, `assets/media/ui-demo.poster.jpg` | the two recorded sessions, their posters and the text of one | recorded from the real tools by the page's scripts (`record_demo.sh`, the cancho-ui demo); EUPL-1.2 |

## Islands (`site/islands/`)

`rocks.js` and `scene.js` are the page's own `assets/rocks.js` and `assets/scene.js`, byte for byte (the hero and the dividers' stones are drawn by them; EUPL-1.2, the page's). The other files there are written for the port
(the glue of each frame and its style: the rules are the page's `assets/site.css`, with the frame's own variables for light and dark). Each is named by its SHA-256 in `home.cui`, and the compiler checks it.

`map.js` and `map.css` (the ecosystem map, B2b) are the page's `assets/map.js` and the map's rules of `assets/site.css` (EUPL-1.2, the page's) **cut down**: the walk (the hiker, the camera, the typed sessions, the charts) is removed (`dropped.md` 14), and the island is given its data as a prop and tells the program which rock was chosen. The changes are the removals and the glue (`cancho.host`, `cancho.onprops`, `cancho.emit`); the drawing code is the page's. It uses `rocks.js` (the page's, unchanged) and the Caveat font of `fonts/` for the names. The map is the one **open** island of the port: plain JavaScript in the page's origin (`README.md`, the authority review).

## Text and data

`data/authority.json`: the code lines, captions and verdicts of the three stories of the authority animation, taken from the page's `assets/site.js` and written as data (the positions of the five rocks stay in the island's script). `data/demos.json` (B2b): the page's `data/demos.json`, byte for byte, the recorded sessions and trails of the map (copied, not regenerated; it is recorded by the page's `scripts/record_demos.py`). `data/ecosystem.json` (SHA-256 `870d4eb8e3bc5a1c22bb33b9b4ae72c6192251af0c7f283eb29db32b3c65ba52`), `agents.md`, `llms.txt`, `llms-full.txt`: the page's own, copied. The last three are **generated by the
page's Python script from the first**; here they are copies and are not regenerated (decision 4 of `docs/www.md` section 10: they stay generated by the page's script for now).
