# AIO Peripherals

Um app de desktop para configurar mouses e teclados de várias marcas num lugar só, sem precisar
de cada software do fabricante. Feito com [Tauri 2](https://tauri.app) (Rust) + React.

*One desktop app to configure mice and keyboards from several brands, without each vendor's
software. The UI is available in Portuguese and English (switch in the top bar).*

> ⚠️ **Use por sua conta e risco. Sem garantia.** Este software é fornecido "no estado em que se
> encontra", sem garantia de qualquer tipo, e altera configurações direto no firmware dos dispositivos.
> Os autores não se responsabilizam por danos a hardware, perda de configurações, travamentos ou perda
> de garantia do fabricante. Leia os [Termos de Uso](TERMOS.md) antes de usar.
>
> ⚠️ ***Use at your own risk. No warranty.** Provided "as is", without warranty of any kind. See the
> [Terms of Use](TERMOS.md).*

## Dispositivos suportados

| Marca | Modelos | O que dá pra configurar | Status |
|---|---|---|---|
| Logitech | Mouses HID++ 2.0 (testado: G Pro Wireless, receptor Lightspeed) | DPI, polling, bateria, RGB, memória interna | ✅ testado |
| AULA | WIN 60 HE (teclado magnético) | Atuação por tecla, Rapid Trigger, teste de curso ao vivo, iluminação e cor por tecla, polling | ✅ testado |
| HyperX | Pulsefire Haste 2 / Haste 2 Wireless | DPI, polling, cor do logo, angle snapping, LOD, bateria | ✅ testado |
| DeLUX | M900 Pro 8K (receptor LXDDZ `1d57:fa65`) | DPI, polling até 8 kHz, angle snap, ripple, motion sync, LOD, tempo de resposta, bateria | ⚠️ parcial |
| DeLUX / ATK / VXE | Mouses na plataforma Compx (`VID 3554`) | DPI, polling, LED de DPI, sensor, bateria | 🧪 experimental |
| Razer | Mouses e teclados (`VID 1532`) | DPI, polling (inclusive HyperPolling), bateria, brilho e efeitos | 🧪 experimental |
| ZOWIE | Mouses (`VID 1af3`) | Apenas detecção (são configurados no próprio mouse) | ℹ️ detecção |

✅ testado em hardware real · ⚠️ testado em parte · 🧪 implementado a partir de documentação, sem hardware para testar.

> **Aviso:** o app conversa diretamente com o firmware dos dispositivos. Os drivers experimentais
> foram escritos sem o hardware em mãos e podem enviar comandos incorretos. Evite deixar o software
> oficial da marca aberto ao mesmo tempo (os dois disputam o dispositivo). Veja os [Termos de Uso](TERMOS.md).

## Como rodar

Pré-requisitos (Windows): [Node.js](https://nodejs.org), [Rust](https://rustup.rs) e o
Visual Studio Build Tools com o workload "Desktop development with C++"
([pré-requisitos do Tauri](https://tauri.app/start/prerequisites/)).

```bash
npm install
npm run tauri dev
```

Para gerar o instalador: `npm run tauri build`.

Ferramentas de desenvolvimento (em `src-tauri`):

```bash
cargo run --example probe                        # lista os dispositivos detectados em JSON
cargo run --example hid_descriptor -- 03f0 0f98  # descritor HID de um VID/PID
cargo test --lib                                 # testes dos protocolos
```

## Estrutura

- `src-tauri/src/devices/` — um driver por família (`logitech.rs`, `aula.rs`, `hyperx.rs`,
  `compx.rs`, `lxd.rs`, `razer.rs`, `zowie.rs`)
- `src/` — interface React; `src/i18n.ts` tem as traduções e `src/devices.ts` o catálogo de fotos

## Créditos dos protocolos

- Logitech HID++ 2.0: documentação pública e projetos como [Solaar](https://github.com/pwr-Solaar/Solaar) e [libratbag](https://github.com/libratbag/libratbag)
- Razer: protocolo documentado pelo [OpenRazer](https://github.com/openrazer/openrazer)
- HyperX Haste 2: [haste2ctl](https://github.com/fspy/haste2ctl), e capturas próprias do NGENUITY (angle snapping, LOD, bateria)
- Receptor LXDDZ 8K: notas em [OpenMouse-Project/mouse-protocol#171](https://github.com/OpenMouse-Project/mouse-protocol/issues/171)
- AULA e Compx: análise dos drivers web oficiais das marcas

## Termos de uso

O uso do app está sujeito aos [Termos de Uso](TERMOS.md): uso por conta e risco, sem garantia e sem
responsabilidade dos autores por danos. O app mostra esse aviso na primeira vez que é aberto.

## Marcas e imagens

Logitech, Razer, HyperX, AULA, DeLUX, ATK, VXE e ZOWIE são marcas dos seus respectivos donos.
Este projeto não é afiliado a nenhuma delas. As fotos dos produtos em `src/assets/devices/`
pertencem aos fabricantes e estão aqui só para identificar os dispositivos no app.
