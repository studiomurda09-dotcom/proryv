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
let track = null;
const samples = [];
file.onReady = (info) => {
  track = info.videoTracks[0];
  file.setExtractionOptions(track.id, null, { nbSamples: 1e9 });
  file.start();
};
file.onSamples = (_id, _user, s) => samples.push(...s);
file.appendBuffer(ab);
file.flush();
if (!track || !samples.length) throw new Error('в файле нет видеодорожки');

// Параметры кодека (avcC) — без них плеер не раскодирует поток.
const entry = file.getTrackById(track.id).mdia.minf.stbl.stsd.entries[0];
const ds = new MP4Box.DataStream(undefined, 0, MP4Box.DataStream.BIG_ENDIAN);
entry.avcC.write(ds);
const description = new Uint8Array(ds.buffer, 8); // без заголовка бокса

const muxer = new Muxer({
  target: new ArrayBufferTarget(),
  video: { codec: 'avc', width: track.video.width, height: track.video.height },
  fastStart: 'in-memory',
  firstTimestampBehavior: 'offset',
});
samples.forEach((s, i) => {
  const us = (v) => Math.round((v * 1e6) / s.timescale);
  muxer.addVideoChunkRaw(new Uint8Array(s.data), s.is_sync ? 'key' : 'delta', us(s.dts), us(s.duration),
    i === 0 ? { decoderConfig: { codec: track.codec, codedWidth: track.video.width, codedHeight: track.video.height, description } } : undefined,
    us(s.cts - s.dts));
});
muxer.finalize();
writeFileSync(output, Buffer.from(muxer.target.buffer));
const secs = (samples.at(-1).dts - samples[0].dts) / samples[0].timescale;
console.log(`${output}: ${track.video.width}×${track.video.height}, ${samples.length} кадров, ${secs.toFixed(1)} с, ~${(samples.length / secs).toFixed(0)} к/с`);
