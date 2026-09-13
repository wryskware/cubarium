// Local file viewer for the actual sail meal frame pairs. No server or live state.
import {readFile, writeFile} from 'node:fs/promises';
import {join} from 'node:path';
import assert from 'node:assert/strict';
const dir=process.argv[2];assert(dir&&process.argv.length===3,'CAPTURE_DIRECTORY');
const report=JSON.parse(await readFile(join(dir,'report.json'),'utf8'));
assert(['stable-body-plus-fin4','body-hold-only'].includes(report.candidate));assert.equal(report.frame_pairs,933);
const sails=JSON.parse(await readFile(join(dir,'sails.json'),'utf8'));
let template=await readFile(new URL('./sail_meal_viewer.html',import.meta.url),'utf8');
if(report.candidate==='body-hold-only')template=template.replaceAll('stable sail + fin4','body hold only').replaceAll('Stable body + fin4','Body hold only');
await writeFile(join(dir,'viewer.html'),template.replace('__SAIL_ROWS__',JSON.stringify(sails)),{flag:'wx'});
