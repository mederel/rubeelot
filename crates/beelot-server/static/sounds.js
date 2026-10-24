// Game sound effects, synthesised with the Web Audio API so no audio assets are needed.
// They respect the "Sound effects" setting and stay silent until the browser allows audio.
const soundSettingKey = "beelot.sound-enabled";
let audioContext = null;

function soundEnabled() {
  try {
    return window.localStorage.getItem(soundSettingKey) !== "false";
  } catch (_) {
    return true;
  }
}

function audio() {
  if (!audioContext) {
    const AudioContextClass = window.AudioContext || window.webkitAudioContext;
    if (!AudioContextClass) return null;
    audioContext = new AudioContextClass();
  }
  if (audioContext.state === "suspended") audioContext.resume().catch(() => {});
  return audioContext;
}

function tone(ctx, { freq, end = freq, start = 0, duration = 0.12, type = "sine", volume = 0.12 }) {
  const begin = ctx.currentTime + start;
  const oscillator = ctx.createOscillator();
  const gain = ctx.createGain();
  oscillator.type = type;
  oscillator.frequency.setValueAtTime(freq, begin);
  oscillator.frequency.exponentialRampToValueAtTime(end, begin + duration);
  gain.gain.setValueAtTime(0.0001, begin);
  gain.gain.exponentialRampToValueAtTime(volume, begin + 0.01);
  gain.gain.exponentialRampToValueAtTime(0.0001, begin + duration);
  oscillator.connect(gain).connect(ctx.destination);
  oscillator.start(begin);
  oscillator.stop(begin + duration + 0.02);
}

function noise(ctx, { start = 0, duration = 0.08, frequency = 2000, volume = 0.15, q = 1 }) {
  const begin = ctx.currentTime + start;
  const buffer = ctx.createBuffer(1, Math.ceil(ctx.sampleRate * duration), ctx.sampleRate);
  const samples = buffer.getChannelData(0);
  for (let index = 0; index < samples.length; index += 1) samples[index] = Math.random() * 2 - 1;
  const source = ctx.createBufferSource();
  const filter = ctx.createBiquadFilter();
  const gain = ctx.createGain();
  source.buffer = buffer;
  filter.type = "bandpass";
  filter.frequency.value = frequency;
  filter.Q.value = q;
  gain.gain.setValueAtTime(volume, begin);
  gain.gain.exponentialRampToValueAtTime(0.0001, begin + duration);
  source.connect(filter).connect(gain).connect(ctx.destination);
  source.start(begin);
}

const arpeggio = (ctx, notes, options = {}) => notes.forEach((freq, index) =>
  tone(ctx, { freq, start: index * 0.09, duration: 0.2, volume: 0.1, ...options }));

const sounds = {
  deal: (ctx) => noise(ctx, { duration: 0.09, frequency: 3200, volume: 0.1 }),
  card: (ctx) => {
    noise(ctx, { duration: 0.05, frequency: 1400, volume: 0.18, q: 0.8 });
    tone(ctx, { freq: 180, end: 90, duration: 0.07, volume: 0.1 });
  },
  bid: (ctx) => {
    tone(ctx, { freq: 660, duration: 0.1 });
    tone(ctx, { freq: 880, start: 0.07, duration: 0.14 });
  },
  pass: (ctx) => tone(ctx, { freq: 330, end: 260, duration: 0.14, type: "triangle" }),
  coinche: (ctx) => {
    noise(ctx, { duration: 0.12, frequency: 900, volume: 0.2 });
    tone(ctx, { freq: 220, end: 110, duration: 0.25, type: "sawtooth", volume: 0.1 });
  },
  trickWin: (ctx) => {
    tone(ctx, { freq: 523, duration: 0.12 });
    tone(ctx, { freq: 784, start: 0.09, duration: 0.2 });
  },
  roundWin: (ctx) => arpeggio(ctx, [523, 659, 784, 1047]),
  roundLose: (ctx) => arpeggio(ctx, [392, 330, 262], { type: "triangle" }),
  matchWin: (ctx) => {
    arpeggio(ctx, [523, 659, 784, 1047, 784, 1047]);
    [523, 659, 784].forEach((freq) => tone(ctx, { freq, start: 0.6, duration: 0.6, volume: 0.07 }));
  },
  correct: (ctx) => {
    tone(ctx, { freq: 660, duration: 0.1 });
    tone(ctx, { freq: 990, start: 0.09, duration: 0.18 });
  },
  wrong: (ctx) => tone(ctx, { freq: 200, end: 150, duration: 0.22, type: "sawtooth", volume: 0.07 })
};

function playSound(name) {
  if (!soundEnabled() || !sounds[name]) return;
  try {
    const ctx = audio();
    if (ctx) sounds[name](ctx);
  } catch (_) { /* audio is a nicety; never break the game */ }
}

// Sound for a call bubble ("Pass", "Coinche!", "80 Hearts", "Spades", and their French forms).
function playCallSound(call) {
  if (/^pass/i.test(call)) playSound("pass");
  else if (/^coinche/i.test(call)) playSound("coinche");
  else playSound("bid");
}
