// Two previously rendered fixed candidates, not a parameter sweep. Writes stdout only.
import {readFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const [bodyPath,combinedPath]=process.argv.slice(2);assert(bodyPath&&combinedPath&&process.argv.length===4);
const body=JSON.parse(await readFile(bodyPath,'utf8')),combined=JSON.parse(await readFile(combinedPath,'utf8'));
assert.equal(body.candidate,'body-hold-only');assert.equal(combined.candidate,'stable-body-plus-fin4');
const rows=[];
const pick=m=>({alpha_mean:m.alpha_area.mean,alpha_sd:m.alpha_area.stddev,solid_mean:m.alpha_ge_nine_tenths_pixels.mean,half_mean:m.alpha_ge_half_pixels.mean,luma_mean:m.linear_luma_sum.mean,peak_mean:m.peak_luma.mean,roughness_mean:m.second_difference_l1.mean});
for(let i=0;i<4;i++){
  const b=body.cases[i],c=combined.cases[i];assert.equal(b.case.clip,c.case.clip);
  assert(b.case.point_body_and_bud_alpha_by_frame.every(v=>b.case.clip!=='move'||v===28));
  for(let j=0;j<b.scenes.length;j++){
    const bs=b.scenes[j],cs=c.scenes[j];assert.equal(bs.scene,cs.scene);assert.equal(bs.scale,cs.scale);
    assert.deepEqual(bs.nearest,cs.nearest,'same original and trajectory must reproduce exactly');
    if(b.case.clip!=='move')assert.deepEqual(bs.fin4,bs.nearest,'body-only must preserve every non-move sample');
    rows.push({state:b.case.clip,scene:bs.scene,scale:bs.scale,original:pick(bs.nearest),body_only:pick(bs.fin4),combined:pick(cs.fin4)});
  }
}
console.log(JSON.stringify({passed:true,scope:'Same original verified across both measurements; body-only non-move output exact. All48 state/scale/scene rows, not a selected subset.',rows},null,2));
