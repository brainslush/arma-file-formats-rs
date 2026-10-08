use std::{fs::File, io::BufReader};

use arma_file_formats::real_virtuality::wrp::{MapData, MapInfo, OPRW, WVR8};
use serial_test::serial;

const INPUT_PATH_PREFIX: &str = "./tests/real_virtuality/test-data/wrp_in/";

#[test]
fn test_defaults() {
    OPRW::default();
}

#[test]
fn gm_test_summer() {
    // gm_weferlingen_summer
    let file = File::open(format!("{}gm_weferlingen_summer.wrp", INPUT_PATH_PREFIX)).unwrap();
    let mut buf = BufReader::new(file);
    let wrp = OPRW::from_read(&mut buf).unwrap();

    let rivers: Vec<&MapInfo> = wrp
        .map_infos
        .iter()
        .filter(|x| matches!(&x.data, MapData::MapTypeRiver { .. }))
        .collect();

    dbg!(rivers.len());
}

#[test]
#[serial]
fn tempelan_wrp() {
    // Tembelan.wrp
    let mut file = File::open(format!("{}Tembelan.wrp", INPUT_PATH_PREFIX)).unwrap();

    let _ = OPRW::from_read(&mut file).unwrap();
}

#[test]
#[serial]
fn stratis_wrp() {
    let mut file = File::open(format!("{}Stratis.wrp", INPUT_PATH_PREFIX)).unwrap();

    let wrp = OPRW::from_read(&mut file).unwrap();
    dbg!(&wrp.map_infos.len());
    dbg!(&wrp.mountains[700]);
    dbg!(&wrp.mountains.len());
    dbg!(&wrp.classed_models.as_ref().unwrap().last());

    assert_eq!(wrp.mountains.len(), 1408);
    assert_eq!(wrp.texures.len(), 479);

    assert_eq!(
        wrp.texures.last().unwrap().texture_filename.to_string(),
        "a3\\map_stratis\\data\\layers\\p_007-007_l02_l04_n.rvmat"
    );
    assert_eq!(
        wrp.models.last().unwrap().to_string(),
        "a3\\signs_f\\signspecial\\signspec_kaminofiringrange_f.p3d"
    );
    assert_eq!(
        wrp.classed_models
            .unwrap()
            .last()
            .unwrap()
            .model_path
            .to_string(),
        "a3\\structures_f\\mil\\cargo\\cargo_house_v1_f.p3d"
    );
    assert_eq!(wrp.max_object_id, 163952);

    assert_eq!(wrp.road_net.len(), 182);
    assert_eq!(
        wrp.road_net
            .last()
            .unwrap()
            .road_parts
            .first()
            .unwrap()
            .p3d_path
            .as_ref()
            .unwrap()
            .to_string(),
        "a3\\structures_f\\bridges\\bridge_01_f.p3d"
    );

    assert!(wrp.objects.iter().any(|x| x.object_id == 156204));

    assert_eq!(wrp.app_id.unwrap_or_default(), 107410);

    assert_eq!(wrp.map_infos.len(), 80420);

    let offsets: Vec<u32> = wrp
        .map_infos
        .iter()
        .scan(0, |at, m| {
            let start = *at;
            *at += m.byte_len();
            Some(start)
        })
        .collect();
    let total: u32 = wrp.map_infos.iter().map(MapInfo::byte_len).sum();
    assert_eq!(total, wrp.size_of_map_info);

    let leaves = wrp.map_object_offsets.leaves(256);
    assert!(leaves.windows(2).all(|w| w[0].value_u32() <= w[1].value_u32()));
    assert!(leaves.iter().all(|l| l.value_u32() <= total));

    // Entries with coordinates must sit in the leaf whose offset range holds them.
    let cell = wrp.layer_cell_size.unwrap();
    let mut checked = 0;
    for (m, &at) in wrp.map_infos.iter().zip(&offsets) {
        let MapData::MapType1 { x, y, .. } = m.data else {
            continue;
        };
        let owner = leaves
            .iter()
            .enumerate()
            .rfind(|(k, l)| {
                l.value_u32() <= at && leaves.get(k + 1).is_none_or(|n| n.value_u32() > at)
            })
            .map(|(_, l)| *l)
            .unwrap();
        let (cx, cy) = ((x / cell).floor() as i64, (y / cell).floor() as i64);
        let (x0, y0, s) = (owner.x as i64, owner.y as i64, owner.size as i64);
        assert!(
            (x0 - 1..=x0 + s).contains(&cx) && (y0 - 1..=y0 + s).contains(&cy),
            "entry at ({x}, {y}) is outside its leaf {owner:?}"
        );
        checked += 1;
    }
    assert_eq!(checked, 69185);
}

#[test]
#[serial]
fn ivachev_wrp() {
    let file = File::open(format!("{}Ivachev.wrp", INPUT_PATH_PREFIX)).unwrap();
    let mut reader = BufReader::new(file);

    let wrp = OPRW::from_read(&mut reader).unwrap();

    dbg!(&wrp.app_id);
}

#[test]
#[serial]
fn fjaderholmarna_test() {
    let mut file = File::open(format!("{}fjaderholmarna.wrp", INPUT_PATH_PREFIX)).unwrap();
    let _ = OPRW::from_read(&mut file).unwrap();
}

#[test]
#[serial]
fn angel_test() {
    let mut file = File::open(format!("{}angel.wrp", INPUT_PATH_PREFIX)).unwrap();
    let wvr8 = WVR8::from_read(&mut file).unwrap();
    assert_eq!(wvr8.rvmat_layer.materials.len(), 238);
    assert_eq!(wvr8.objects.len(), 62249);
}
