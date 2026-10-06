// Tiny i18n: Portuguese strings are the keys; English comes from this table.
// Strings sent by the backend (notes, zone and effect names) go through the same lookup.

export type Lang = "pt" | "en";

const EN: Record<string, string> = {
  // shell
  "Periféricos": "Peripherals",
  "Procurar dispositivos": "Scan for devices",
  Mouses: "Mice",
  Teclados: "Keyboards",
  dormindo: "asleep",
  "só detecção": "detect only",
  conectado: "connected",
  "Nenhum dispositivo encontrado. Conecte algo e clique em atualizar.": "No devices found. Plug something in and hit refresh.",
  "Procurando dispositivos…": "Looking for devices…",
  "Selecione um dispositivo": "Select a device",
  Minimizar: "Minimize",
  Maximizar: "Maximize",
  Fechar: "Close",
  Idioma: "Language",
  // device detail
  "Em breve": "Coming soon",
  Offline: "Offline",
  Bateria: "Battery",
  Carregando: "Charging",
  Sensibilidade: "Sensitivity",
  "Taxa de polling": "Polling rate",
  "Reconectando…": "Reconnecting…",
  "Memória interna": "Onboard memory",
  "O mouse usa o perfil salvo na memória dele. Mudar o polling ou a iluminação passa para o modo software.":
    "The mouse uses the profile saved in its memory. Changing polling or lighting switches to software mode.",
  "Modo software: as configurações vêm deste app e se perdem quando o mouse desliga. Ative para voltar ao perfil salvo.":
    "Software mode: settings come from this app and reset when the mouse powers off. Turn on to restore the saved profile.",
  "Faixa de {min} a {max}, em passos de {step}.": "Range {min}–{max} in steps of {step}.",
  // keyboard
  Fixo: "Static",
  "RT separado": "RT · split",
  Atuação: "Actuation",
  Iluminação: "Lighting",
  "Teste ao vivo": "Live test",
  "Clique ou arraste sobre as teclas para pintar com o pincel.": "Click or drag over keys to paint them with the brush.",
  "Escolha o efeito Personalizado (ou comece a pintar) para colorir tecla por tecla.":
    "Pick the Custom effect (or just start painting) to colour individual keys.",
  "Aperte qualquer tecla para ver o quanto ela desce.": "Press any key to see how far it goes down.",
  "Zerar picos": "Reset peaks",
  "Clique ou arraste sobre as teclas para selecionar. Sem seleção = todas as teclas.":
    "Click or drag across keys to select. No selection = all keys.",
  "{n} tecla selecionada": "{n} key selected",
  "{n} teclas selecionadas": "{n} keys selected",
  Todas: "All",
  Limpar: "Clear",
  "Valores = ponto de atuação (mm)": "Values = actuation point (mm)",
  Curso: "Travel",
  "Passou do ponto de atuação": "Past actuation point",
  "Valores = profundidade atual (mm)": "Values = current depth (mm)",
  "Cores = layout do efeito Personalizado (preto = apagada)": "Colours shown = Custom effect layout (black = off)",
  "Curso ao vivo": "Live travel",
  Tecla: "Key",
  Espaço: "Space",
  Pico: "Peak",
  Modo: "Mode",
  "Linha verde = ponto de atuação, linha branca = pressão mais funda. Curso total {mm} mm.":
    "Green line = actuation point, white line = deepest press. Full travel {mm} mm.",
  "Todas as teclas": "All keys",
  "Valores diferentes: editar iguala todas": "Mixed values — editing sets all to the same",
  "Ponto de atuação": "Actuation point",
  "Sensibilidade ao apertar": "Press sensitivity",
  "Sensibilidade do Rapid Trigger": "Rapid Trigger sensitivity",
  "Sensibilidade ao soltar": "Release sensitivity",
  "Curso de {min} a {max} mm.": "Travel range {min}–{max} mm.",
  // lighting
  Pincel: "Brush",
  "Clique ou arraste nas teclas para pintar": "Click or drag over keys to paint",
  Apagado: "Off",
  "Cor personalizada": "Custom colour",
  "Pintar todas": "Fill all with brush",
  "Apagar todas": "Clear all",
  "Arco-íris": "Rainbow",
  Direção: "Direction",
  Brilho: "Brightness",
  Velocidade: "Speed",
  Desligado: "Off",
  Ciclo: "Cycle",
  Respirar: "Breathe",
  "Sincronizar zonas": "Sync zones",
  "→ Direita": "→ Right",
  "← Esquerda": "← Left",
  "↑ Cima": "↑ Up",
  "↓ Baixo": "↓ Down",
  "Para fora": "Outward",
  "Para dentro": "Inward",
  "↻ Horário": "↻ Clockwise",
  "↺ Anti-horário": "↺ Counter",
  "O mouse não informa a iluminação atual, então aqui aparece o último efeito definido pelo app.":
    "The mouse can't report its current lighting, so this shows the last effect set here.",
  // AULA effect names
  Onda: "Wave",
  Neon: "Neon",
  Cintilante: "Twinkle",
  Radar: "Radar",
  "Onda automática": "Auto Ripple",
  Listras: "Striation",
  Reativo: "Reactive",
  Aurora: "Aurora",
  Cruz: "Cross",
  "Resposta rápida": "Speed Respond",
  Fogos: "Fireworks",
  Marola: "Ripple",
  Personalizado: "Custom",
  // zones
  Principal: "Primary",
  Logo: "Logo",
  "LED de DPI": "DPI LED",
  Scroll: "Scroll wheel",
  Tudo: "All",
  // sensor
  Sensor: "Sensor",
  "Endireita movimentos quase horizontais ou verticais.": "Straightens nearly horizontal/vertical movements.",
  "Suaviza a tremedeira do sensor em DPI alto (adiciona um pouco de latência).":
    "Smooths sensor jitter at high DPI (adds a little latency).",
  "Sincroniza a leitura do sensor com o polling USB para um rastreio mais estável.":
    "Aligns sensor reads with USB polling for more consistent tracking.",
  "Altura de levantamento (LOD)": "Lift-off distance",
  "Debounce do clique": "Click debounce",
  "Menor é mais rápido, mas baixo demais pode gerar cliques duplos.": "Lower is faster, but too low can cause accidental double clicks.",
  // terms
  "Termos de uso": "Terms of use",
  "Este app altera configurações direto no firmware dos seus mouses, teclados e receptores.":
    "This app changes settings directly in the firmware of your mice, keyboards and receivers.",
  "Use por sua conta e risco.": "Use at your own risk.",
  "Alguns drivers são experimentais e foram feitos sem o aparelho para testar.":
    "Some drivers are experimental and were written without the hardware to test on.",
  "Sem garantia.": "No warranty.",
  'O software é fornecido "no estado em que se encontra", sem garantia de qualquer tipo.':
    'The software is provided "as is", without warranty of any kind.',
  "Os autores não se responsabilizam por danos ao hardware, perda de configurações, travamentos ou perda da garantia do fabricante.":
    "The authors are not liable for hardware damage, lost settings, crashes or loss of the manufacturer's warranty.",
  "Não é afiliado a nenhuma das marcas suportadas.": "Not affiliated with any of the supported brands.",
  "O texto completo está no arquivo TERMOS.md do repositório.": "The full text is in the repository's TERMOS.md file.",
  Sair: "Exit",
  "Li e aceito": "I have read and accept",
  // backend notes
  "Receptor encontrado, mas o mouse está desligado ou dormindo. Mexa nele e atualize.":
    "Receiver found, but the mouse is off or asleep. Move it and refresh.",
  "O mouse está desligado ou dormindo. Mexa nele e atualize.": "The mouse is off or asleep. Move it and refresh.",
  "Suporte experimental (protocolo OpenRazer), ainda não testado com este modelo.":
    "Experimental support (OpenRazer protocol), not yet tested with this model.",
  "Os mouses ZOWIE são configurados no próprio mouse (botões e chaves embaixo dele). Não há protocolo público para mudar isso pelo USB.":
    "ZOWIE mice are configured on the mouse itself (buttons and switches underneath). There is no public protocol to change this over USB.",
  "Detectado, mas o layout de teclas deste modelo ainda não foi mapeado.": "Detected, but this model's key layout is not mapped yet.",
  // backend errors
  "comando não suportado por este modelo": "command not supported by this model",
  "o aparelho recusou o comando": "the device rejected the command",
  "o aparelho não respondeu (desligado?)": "the device did not respond (off?)",
  "sem resposta": "no response",
};

const STORAGE_KEY = "lang";

function initial(): Lang {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "pt" || saved === "en") return saved;
  } catch {
    /* storage unavailable */
  }
  return navigator.language.toLowerCase().startsWith("pt") ? "pt" : "en";
}

let lang: Lang = initial();

export const getLang = () => lang;

export function setLang(next: Lang) {
  lang = next;
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {
    /* storage unavailable */
  }
  document.documentElement.lang = next === "pt" ? "pt-BR" : "en";
}

/** Translates a Portuguese string; `{name}` placeholders are filled from `vars`. Unknown strings pass through. */
export function tr(pt: string, vars?: Record<string, string | number>): string {
  let s = lang === "en" ? EN[pt] ?? pt : pt;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.split(`{${k}}`).join(String(v));
  return s;
}

/** Same as tr() but for backend strings that may embed a number (e.g. "Zona 3"). */
export function trName(name: string): string {
  const m = name.match(/^Zona (\d+)$/);
  if (m) return lang === "en" ? `Zone ${m[1]}` : name;
  return tr(name);
}
