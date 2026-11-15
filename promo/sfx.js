'use strict';
/* =====================================================================
   Звуки промо-ролика: всё синтезируется Web Audio, без готовых сэмплов.
   SFX.play(id) вызывается из cue() режиссёра — звук звучит ровно в момент события.
   ===================================================================== */
const SFX = (() => {
  let ctx, out, rev, noise, dest, pad;
  let rows = 0;

  function init() {
    ctx = new AudioContext({ sampleRate: 48000 });
    dest = ctx.createMediaStreamDestination();
    const comp = ctx.createDynamicsCompressor();
    comp.threshold.value = -14; comp.ratio.value = 3; comp.attack.value = 0.004; comp.release.value = 0.2;
    out = ctx.createGain(); out.gain.value = 0.85;
    out.connect(comp); comp.connect(dest); comp.connect(ctx.destination);
    // Ревербератор: импульс — затухающий стерео-шум.
    rev = ctx.createConvolver();
    const len = ctx.sampleRate * 2.4, ir = ctx.createBuffer(2, len, ctx.sampleRate);
    for (let c = 0; c < 2; c++) {
      const d = ir.getChannelData(c);
      for (let i = 0; i < len; i++) d[i] = (Math.random() * 2 - 1) * Math.pow(1 - i / len, 3.2);
    }
    rev.buffer = ir;
    const revOut = ctx.createGain(); revOut.gain.value = 0.55;
    rev.connect(revOut); revOut.connect(out);
    noise = ctx.createBuffer(1, ctx.sampleRate * 2, ctx.sampleRate);
    const nd = noise.getChannelData(0);
    for (let i = 0; i < nd.length; i++) nd[i] = Math.random() * 2 - 1;
    return ctx.resume();
  }

  const T = () => ctx.currentTime + 0.005;
  /** Узел громкости с огибающей и посылом в реверб. */
  function voice(t, { a = 0.003, peak = 0.5, d = 0.2, wet = 0.15, pan = 0 } = {}) {
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(peak, t + a);
    g.gain.exponentialRampToValueAtTime(0.0001, t + a + d);
    const p = ctx.createStereoPanner(); p.pan.value = pan;
    g.connect(p); p.connect(out);
    if (wet) { const s = ctx.createGain(); s.gain.value = wet; p.connect(s); s.connect(rev); }
    return g;
  }
  function osc(type, f, t, dur, to) {
    const o = ctx.createOscillator(); o.type = type; o.frequency.setValueAtTime(f, t);
    if (to) o.frequency.exponentialRampToValueAtTime(to, t + dur);
    o.start(t); o.stop(t + dur + 0.05);
    return o;
  }
  function noiseSrc(t, dur) {
    const s = ctx.createBufferSource(); s.buffer = noise; s.loop = true;
    s.start(t, Math.random()); s.stop(t + dur + 0.05);
    return s;
  }
  function filt(type, f, q = 0.7) { const b = ctx.createBiquadFilter(); b.type = type; b.frequency.value = f; b.Q.value = q; return b; }

  /* ---------- Элементарные звуки ---------- */
  function click(t = T(), { f = 3200, peak = 0.3, body = 1900 } = {}) {
    const n = noiseSrc(t, 0.02), bp = filt('bandpass', f, 1.2);
    n.connect(bp); bp.connect(voice(t, { a: 0.001, peak, d: 0.018, wet: 0.08 }));
    osc('sine', body, t, 0.03, body * 0.7).connect(voice(t, { a: 0.001, peak: peak * 0.5, d: 0.03, wet: 0 }));
  }
  function heavyClick(t = T()) {
    click(t, { f: 1500, peak: 0.45, body: 900 });
    osc('sine', 160, t, 0.12, 60).connect(voice(t, { a: 0.002, peak: 0.6, d: 0.12, wet: 0.05 }));
  }
  function toggle(t = T()) {
    click(t, { f: 2600, peak: 0.25, body: 1500 });
    click(t + 0.045, { f: 4200, peak: 0.22, body: 2400 });
    osc('sine', 1320, t + 0.05, 0.08).connect(voice(t + 0.05, { a: 0.004, peak: 0.12, d: 0.08, wet: 0.2 }));
  }
  function key(t = T()) {
    const n = noiseSrc(t, 0.01), bp = filt('bandpass', 2000 + Math.random() * 1500, 1.5);
    n.connect(bp); bp.connect(voice(t, { a: 0.001, peak: 0.14 + Math.random() * 0.06, d: 0.012, wet: 0.03, pan: Math.random() * 0.4 - 0.2 }));
    const lp = filt('lowpass', 600); noiseSrc(t, 0.02).connect(lp);
    lp.connect(voice(t, { a: 0.001, peak: 0.1, d: 0.02, wet: 0 }));
  }
  function blip(i, t = T()) {
    const f = 660 * Math.pow(2, (i * 3) / 12);
    osc('triangle', f, t, 0.09).connect(voice(t, { a: 0.003, peak: 0.22, d: 0.09, wet: 0.25, pan: -0.3 + i * 0.2 }));
    osc('sine', f * 2, t, 0.05).connect(voice(t, { a: 0.002, peak: 0.06, d: 0.05, wet: 0.1 }));
  }
  function pop(t = T(), from = 280, to = 950) {
    osc('sine', from, t, 0.06, to).connect(voice(t, { a: 0.004, peak: 0.35, d: 0.13, wet: 0.2 }));
  }
  function chime(t = T()) {
    [1046.5, 1318.5, 1568, 2093].forEach((f, i) => {
      const s = t + i * 0.055;
      osc('sine', f, s, 1.6).connect(voice(s, { a: 0.006, peak: 0.16, d: 1.5, wet: 0.5, pan: -0.3 + i * 0.2 }));
      osc('triangle', f / 2, s, 0.6).connect(voice(s, { a: 0.006, peak: 0.05, d: 0.5, wet: 0.3 }));
    });
  }
  function ding(t = T()) {
    osc('sine', 1760, t, 0.9).connect(voice(t, { a: 0.003, peak: 0.16, d: 0.8, wet: 0.5 }));
    osc('sine', 2637, t, 0.5).connect(voice(t, { a: 0.003, peak: 0.07, d: 0.45, wet: 0.5 }));
  }
  /** Свуш: шум через полосовой фильтр, частота и панорама едут. */
  function whoosh(t = T(), { dur = 0.6, from = 350, to = 3200, peak = 0.35, panFrom = -0.6, panTo = 0.6, wet = 0.3 } = {}) {
    const n = noiseSrc(t, dur), bp = filt('bandpass', from, 0.9);
    bp.frequency.setValueAtTime(from, t); bp.frequency.exponentialRampToValueAtTime(to, t + dur);
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(peak, t + dur * 0.6);
    g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    const p = ctx.createStereoPanner(); p.pan.setValueAtTime(panFrom, t); p.pan.linearRampToValueAtTime(panTo, t + dur);
    n.connect(bp); bp.connect(g); g.connect(p); p.connect(out);
    const s = ctx.createGain(); s.gain.value = wet; p.connect(s); s.connect(rev);
  }
  function impact(t = T(), big = false) {
    osc('sine', big ? 140 : 120, t, big ? 1.1 : 0.7, 36).connect(voice(t, { a: 0.003, peak: big ? 0.9 : 0.75, d: big ? 1.1 : 0.7, wet: 0.2 }));
    const n = noiseSrc(t, 0.4), lp = filt('lowpass', big ? 1400 : 900);
    n.connect(lp); lp.connect(voice(t, { a: 0.002, peak: 0.5, d: 0.3, wet: 0.6 }));
  }
  function shimmer(t = T(), dur = 1.8) {
    for (let i = 0; i < 9; i++) {
      const s = t + i * 0.06, f = [2093, 2637, 3136, 3951, 4186, 5274][i % 6] * (1 + (Math.random() - 0.5) * 0.01);
      osc('sine', f, s, dur).connect(voice(s, { a: 0.08, peak: 0.035, d: dur, wet: 0.8, pan: Math.random() * 1.4 - 0.7 }));
    }
  }
  function powerUp(t = T()) {
    const o = osc('sawtooth', 70, t, 0.55, 420), lp = filt('lowpass', 300, 4);
    lp.frequency.setValueAtTime(300, t); lp.frequency.exponentialRampToValueAtTime(4500, t + 0.5);
    o.connect(lp); lp.connect(voice(t, { a: 0.05, peak: 0.28, d: 0.55, wet: 0.4 }));
    osc('sine', 55, t, 0.6, 110).connect(voice(t, { a: 0.05, peak: 0.4, d: 0.6, wet: 0 }));
    // Мягкий гул после включения.
    [110, 110.8, 220.4].forEach((f) => osc('sine', f, t + 0.45, 1.6).connect(voice(t + 0.45, { a: 0.15, peak: 0.07, d: 1.4, wet: 0.3 })));
    chime(t + 0.5);
  }
  function ticks(t = T(), dur = 2.2) {
    for (let s = 0; s < dur; s += 0.085 + Math.random() * 0.05) {
      osc('square', 3000 + Math.random() * 2000, t + s, 0.008).connect(voice(t + s, { a: 0.001, peak: 0.035, d: 0.01, wet: 0.05, pan: Math.random() * 1.2 - 0.6 }));
    }
  }
  function glitch(t = T()) {
    for (let i = 0; i < 9; i++) {
      const s = t + i * 0.028;
      osc('square', 180 + Math.random() * 1800, s, 0.022).connect(voice(s, { a: 0.001, peak: 0.09, d: 0.022, wet: 0.05, pan: Math.random() * 1.6 - 0.8 }));
    }
    const n = noiseSrc(t, 0.25), hp = filt('highpass', 2500);
    n.connect(hp); hp.connect(voice(t, { a: 0.002, peak: 0.12, d: 0.22, wet: 0.2 }));
  }
  function rustle(t = T()) {
    const n = noiseSrc(t, 0.3), bp = filt('bandpass', 5000, 0.6);
    n.connect(bp); bp.connect(voice(t, { a: 0.03, peak: 0.08, d: 0.25, wet: 0.3 }));
  }
  /** Тихая атмосфера на весь ролик — не музыка, а «воздух» под звуками. */
  function padStart(t = T()) {
    const g = ctx.createGain(); g.gain.setValueAtTime(0.0001, t); g.gain.exponentialRampToValueAtTime(0.045, t + 2.5);
    const lp = filt('lowpass', 380, 0.5);
    const lfo = osc('sine', 0.08, t, 120); const lg = ctx.createGain(); lg.gain.value = 160; lfo.connect(lg); lg.connect(lp.frequency);
    [55, 55.4, 82.4, 110.2].forEach((f) => osc('sawtooth', f, t, 120).connect(lp));
    lp.connect(g); g.connect(out);
    const s = ctx.createGain(); s.gain.value = 0.4; g.connect(s); s.connect(rev);
    pad = g;
  }
  function padStop(t = T()) { if (pad) { pad.gain.cancelScheduledValues(t); pad.gain.setValueAtTime(pad.gain.value, t); pad.gain.exponentialRampToValueAtTime(0.0001, t + 3.5); } }

  /* ---------- Звук на каждое событие ролика ---------- */
  const MAP = {
    intro: (t) => { padStart(t); impact(t); shimmer(t + 0.05); },
    intro_out: (t) => whoosh(t, { dur: 0.45, from: 2500, to: 500, peak: 0.25 }),
    window_in: (t) => { whoosh(t, { dur: 0.9, from: 250, to: 2600, peak: 0.42, panFrom: 0, panTo: 0 }); osc('sine', 90, t + 0.8, 0.3, 45).connect(voice(t + 0.8, { a: 0.01, peak: 0.45, d: 0.3, wet: 0.1 })); },
    caption_in: (t) => whoosh(t, { dur: 0.4, from: 900, to: 4500, peak: 0.12, panFrom: -0.7, panTo: 0 }),
    caption_out: (t) => whoosh(t, { dur: 0.35, from: 3500, to: 800, peak: 0.06, panFrom: 0, panTo: -0.7 }),
    click: (t) => click(t),
    power_click: (t) => heavyClick(t),
    power_on: (t) => powerUp(t),
    stats_tick: (t) => ticks(t),
    row: (t) => blip(rows++, t),
    success: (t) => chime(t),
    toast: (t) => pop(t),
    toggle_on: (t) => toggle(t),
    scroll: (t) => whoosh(t, { dur: 0.5, from: 600, to: 2000, peak: 0.12, panFrom: 0, panTo: 0 }),
    bar_redraw: (t) => glitch(t),
    search_open: (t) => pop(t, 400, 1200),
    search_results: (t) => rustle(t),
    jump: (t) => { whoosh(t, { dur: 0.45, from: 500, to: 3000, peak: 0.22 }); ding(t + 0.35); },
    lang_switch: (t) => { glitch(t); whoosh(t + 0.05, { dur: 0.6, from: 3000, to: 400, peak: 0.25, panFrom: 0.6, panTo: -0.6 }); },
    window_out: (t) => { whoosh(t, { dur: 0.8, from: 2800, to: 300, peak: 0.3, panFrom: 0, panTo: 0 }); padStop(t); },
    outro: (t) => { impact(t, true); shimmer(t + 0.05, 3); },
  };
  return {
    init,
    get stream() { return dest.stream; },
    play(id) { if (ctx && MAP[id]) MAP[id](T()); },
    key() { if (ctx) key(); },
  };
})();
