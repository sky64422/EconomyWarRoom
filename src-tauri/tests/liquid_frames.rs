use economy_war_room_lib::infrastructure::liquid_capture::{FrameCache, FrameKey};
fn key() -> FrameKey { FrameKey { x:-20,y:100,width:2,height:1,generation:1 } }
#[test]
fn unchanged_frames_skip_only_when_surface_generation_and_region_match() {
 let mut c=FrameCache::default();let p=[0,0,255,0,255,128,0,0];
 let first=c.frame(key(),&p).unwrap();
 assert_eq!(&first[..4],b"LQ01");assert_eq!(&first[12..],&[255,0,0,255,0,128,255,255]);
 assert!(c.frame(key(),&p).unwrap().is_empty());
 let mut alpha=p;alpha[3]=99;assert!(c.frame(key(),&alpha).unwrap().is_empty());
 let mut changed=p;changed[5]=129;assert!(!c.frame(key(),&changed).unwrap().is_empty());
 assert!(!c.frame(FrameKey{generation:2,..key()},&changed).unwrap().is_empty());
 assert!(!c.frame(FrameKey{x:20,..key()},&changed).unwrap().is_empty());
 c.clear();assert!(!c.frame(key(),&changed).unwrap().is_empty());
}
#[test]
fn invalid_frame_lengths_and_dimensions_are_rejected_before_allocation() {
 let mut c=FrameCache::default();
 assert!(c.frame(key(),&[0;4]).is_err());
 assert!(c.frame(FrameKey{width:0,..key()},&[]).is_err());
 assert!(c.frame(FrameKey{width:u32::MAX,height:u32::MAX,..key()},&[]).is_err());
}
#[test]
fn settling_identical_frame_cache_hit_returns_empty_bytes_to_trigger_release() {
    let mut cache = FrameCache::default();
    let pixels = [30, 40, 50, 0, 60, 70, 80, 0];
    let k = key();
    let first = cache.frame(k, &pixels).unwrap();
    assert!(!first.is_empty(), "First frame should produce bytes");

    // Settling frame where desktop hasn't changed produces 0 bytes (cache hit).
    // The Rust capture command and frontend release loop ensure this does not keep exclusion affinity.
    let settling = cache.frame(k, &pixels).unwrap();
    assert!(settling.is_empty(), "Cache hit must return empty bytes");
}
