// Package native paired meal captures for one-request-per-case browser playback.
// Input PNGs, reports and world snapshots are read-only. All output must be new.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
const [source,output]=process.argv.slice(2);
assert(source&&output&&process.argv.length===4,'Usage: node package.mjs CAPTURE_DIRECTORY NEW_VIEWER_DIRECTORY');
const root=resolve(source),out=resolve(output);
assert.notEqual(root,out,'input captures are immutable');
await mkdir(out);
const hashes={};
for(const name of ['seed1-t0-feed','seed8-t0-feed','seed1-noinput']){
  const dir=join(root,name),dest=join(out,name);
  await mkdir(dest);
  const reportBytes=await readFile(join(dir,'report.json'));
  const r=JSON.parse(reportBytes);
  assert.equal(r.kind,'meal-onset-paired-capture-v1');
  assert.equal(r.frames_per_tick,3);
  for(const n of [r.frames_from,r.frames_to,r.frame_pairs_written])assert(Number.isSafeInteger(n)&&n>=0);
  assert(r.frames_from<=r.frames_to&&r.frames_to<r.ticks);
  assert.equal(r.frame_pairs_written,(r.frames_to-r.frames_from+1)*3);
  const bundle={};
  for(const label of ['old','new']){
    bundle[label]=[];
    const digest=createHash('sha256');
    for(let i=0;i<r.frame_pairs_written;i++){
      const b=await readFile(join(dir,label,`frame_${String(r.frames_from*3+i).padStart(5,'0')}.png`));
      assert(b.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])),'not PNG');
      assert.equal(b.readUInt32BE(16),256);assert.equal(b.readUInt32BE(20),128);
      digest.update(b);bundle[label].push(`data:image/png;base64,${b.toString('base64')}`);
    }
    hashes[`${name}/${label}`]={frames:bundle[label].length,concatenated_png_sha256:digest.digest('hex')};
  }
  await writeFile(join(dest,'frames.json'),JSON.stringify(bundle),{flag:'wx'});
  await writeFile(join(dest,'report.json'),reportBytes,{flag:'wx'});
  await writeFile(join(dest,'organisms.jsonl'),await readFile(join(dir,'organisms.jsonl')),{flag:'wx'});
}
await writeFile(join(out,'index.html'),await readFile(fileURLToPath(new URL('./viewer.html',import.meta.url))),{flag:'wx'});
await writeFile(join(out,'package.json'),JSON.stringify({source:root,hashes},null,2),{flag:'wx'});
console.log(out);
