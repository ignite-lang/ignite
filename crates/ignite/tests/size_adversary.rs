//! Adversary, wave 9 unit `size` (story `readiness-binary-size`): cases
//! that read the story's acceptance against the tree.

use std::collections::BTreeMap;

use crate::quality;

/// The story's acceptance: "Row 3.2 of `task quality` shows every release
/// binary at most 10 % over its v0.15.0 value, and the bars are blessed
/// down." The ratchet holds a release cell only to its bar in
/// `quality/baseline.json` (plus the 5 % tolerance), so while a bar stays
/// at v0.16.0's accepted growth the ratchet lets a release binary grow back
/// to it. Every `*/release` bar of 3.2 is at most 110 % of the cell's
/// v0.15.0 record.
#[test]
fn release_bars_of_row_3_2_are_blessed_down_to_v0_15_0() {
    let before: BTreeMap<String, i64> = quality::read_history()
        .into_iter()
        .filter(|record| record.run == "v0.15.0" && record.metric == "3.2")
        .map(|record| (record.target, record.value))
        .collect();
    let baseline = quality::load_baseline();
    let metric = baseline
        .metrics
        .get("3.2")
        .expect("row 3.2 in the baseline");
    let mut checked = 0;
    let mut over = Vec::new();
    for (cell, bar) in &metric.bars {
        if !cell.ends_with("/release") {
            continue;
        }
        checked += 1;
        let reference = *before
            .get(cell)
            .unwrap_or_else(|| panic!("no v0.15.0 record of 3.2 {cell}"));
        let ceiling = reference + reference / 10;
        if bar.value > ceiling {
            over.push(format!(
                "{cell}: bar {} B, v0.15.0 {reference} B, at most {ceiling} B",
                bar.value
            ));
        }
    }
    assert_eq!(checked, 4, "release cells of 3.2 in the baseline");
    assert!(
        over.is_empty(),
        "3.2 release bars not blessed down:\n{}",
        over.join("\n")
    );
}
