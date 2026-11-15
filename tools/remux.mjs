// Перепаковать фрагментированный MP4 из MediaRecorder в обычный (индекс в начале,
// нормальная длительность и перемотка) без перекодирования: node tools/remux.mjs in.mp4 out.mp4
import MP4Box from 'mp4box';
import { Muxer, ArrayBufferTarget } from 'mp4-muxer';
import { readFileSync, writeFileSync } from 'node:fs';

const [input, output] = process.argv.slice(2);
const buf = readFileSync(input);
const ab = buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength);
ab.fileStart = 0;

const file = MP4Box.createFile();
let vt = null, at = null;
const samples = { v: [], a: [] };
file.onReady = (info) => {
  vt = info.videoTracks[0];
  at = info.audioTracks[0] || null;
  file.setExtractionOptions(vt.id, 'v', { nbSamples: 1e9 });
  if (at) file.setExtractionOptions(at.id, 'a', { nbSamples: 1e9 });
  file.start();
};
file.onSamples = (_id, kind, s) => samples[kind].push(...s);
file.appendBuffer(ab);
file.flush();
if (!vt || !samples.v.length) throw new Error('в файле нет видеодорожки');

const stsd = (t) => file.getTrackById(t.id).mdia.minf.stbl.stsd.entries[0];

// Параметры видеокодека (avcC) — без них плеер не раскодирует поток.
const ds = new MP4Box.DataStream(undefined, 0, MP4Box.DataStream.BIG_ENDIAN);
stsd(vt).avcC.write(ds);
const avcC = new Uint8Array(ds.buffer, 8); // без заголовка бокса

// Параметры AAC (AudioSpecificConfig) лежат в esds: ES → DecoderConfig (тег 4) → DecoderSpecificInfo (тег 5).
let asc = null;
if (at) {
  const dcd = stsd(at).esds?.esd?.descs?.find((d) => d.tag === 4);
  asc = dcd?.descs?.find((d) => d.tag === 5)?.data;
  if (!asc) throw new Error('не нашёл параметры AAC');
}

const muxer = new Muxer({
  target: new ArrayBufferTarget(),
  video: { codec: 'avc', width: vt.video.width, height: vt.video.height },
  ...(at ? { audio: { codec: 'aac', numberOfChannels: at.audio.channel_count, sampleRate: at.audio.sample_rate } } : {}),
  fastStart: 'in-memory',
  firstTimestampBehavior: 'cross-track-offset',
});
const us = (v, s) => Math.round((v * 1e6) / s.timescale);
samples.v.forEach((s, i) => muxer.addVideoChunkRaw(new Uint8Array(s.data), s.is_sync ? 'key' : 'delta', us(s.dts, s), us(s.duration, s),
  i === 0 ? { decoderConfig: { codec: vt.codec, codedWidth: vt.video.width, codedHeight: vt.video.height, description: avcC } } : undefined,
  us(s.cts - s.dts, s)));
samples.a.forEach((s, i) => muxer.addAudioChunkRaw(new Uint8Array(s.data), 'key', us(s.cts, s), us(s.duration, s),
  i === 0 ? { decoderConfig: { codec: at.codec, numberOfChannels: at.audio.channel_count, sampleRate: at.audio.sample_rate, description: new Uint8Array(asc) } } : undefined));
muxer.finalize();
writeFileSync(output, Buffer.from(muxer.target.buffer));
const v = samples.v, secs = (v.at(-1).dts - v[0].dts) / v[0].timescale;
console.log(`${output}: ${vt.video.width}×${vt.video.height}, ${v.length} кадров, ${secs.toFixed(1)} с, ~${(v.length / secs).toFixed(0)} к/с`
  + (at ? `, звук AAC ${at.audio.sample_rate} Гц ${at.audio.channel_count} кан.` : ', без звука'));
