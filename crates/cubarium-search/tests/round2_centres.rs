//! Round 2's centres still load under this build's founder manifests: the lifespan and
//! the grazing defence add no sense input (`design/handoffs/voxel-lifespan-defence-2026-09-25.md`).

use cubarium_search::es::voxel::VoxelPolicyFile;
use cubarium_voxel_fauna::Founder;

#[test]
fn round_two_centres_still_load() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/training/centres");
    for (file, founder) in [
        ("r2-frondgrazer-upd160.json", Founder::Browser),
        ("r2-littershredder-upd512.json", Founder::Blind),
    ] {
        let policy = VoxelPolicyFile::load(&dir.join(file)).expect("a policy file");
        assert_eq!(policy.founder().expect("a lineage"), founder);
        assert_eq!(
            policy.digest,
            founder.manifest().digest(),
            "{file}: the manifest moved"
        );
        policy
            .driver()
            .expect("the centre validates against this build");
    }
}
