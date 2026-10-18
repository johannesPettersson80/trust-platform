import * as assert from "assert";

import {
  adsTagRemovalResult,
  buildOfflineAdsRemoveArgs,
} from "../../networkCanvas/offlineComm";

suite("ADS tag removal", () => {
  test("removing a tag runs `trust-runtime ads remove-symbols` and shows its report", () => {
    // The extension host no longer edits ads.toml itself: removing a tag runs the Rust
    // `trust-runtime ads remove-symbols` command and shows the report it prints (design §4.3 B7;
    // specification 25 §5.4). A wrong implementation edits the TOML in TypeScript, calls the old
    // `ads import` regeneration path, drops the connection identity, or hides the Rust report.
    assert.deepStrictEqual(
      buildOfflineAdsRemoveArgs(
        "/home/u/plant/line1",
        { host: "192.168.77.11", target_net_id: "100.67.6.217.1.1" },
        301,
        ["Task 4.Inputs.Var 1"],
      ),
      [
        "ads",
        "remove-symbols",
        "--project",
        "/home/u/plant/line1",
        "--ams-port",
        "301",
        "--host",
        "192.168.77.11",
        "--target-net-id",
        "100.67.6.217.1.1",
        "--path",
        "Task 4.Inputs.Var 1",
        "--json",
      ],
    );

    assert.deepStrictEqual(
      adsTagRemovalResult({
        schema_version: 1,
        applied: true,
        matched: true,
        removed_count: 2,
        restart_required: true,
        files_written: ["ads.toml", "src/generated/ads_generated.st"],
        message: "Removed 2 ADS tags.",
      }),
      {
        applied: true,
        removed_count: 2,
        restart_required: true,
        message: "Removed 2 ADS tags.",
      },
    );
  });
});
