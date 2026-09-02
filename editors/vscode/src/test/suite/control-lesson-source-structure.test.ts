import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

function curriculumRoot(): string {
  const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
    ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
    : path.resolve(__dirname, "..", "..", "..", "..", "..");
  return path.join(repositoryRoot, "examples", "control");
}

function lessonDirectories(): string[] {
  const root = curriculumRoot();
  return fs.readdirSync(root)
    .filter((name) => /^\d{2}-/.test(name))
    .flatMap((moduleName) => {
      const moduleRoot = path.join(root, moduleName);
      return fs.readdirSync(moduleRoot)
        .filter((name) => /^\d{2}-/.test(name))
        .map((name) => path.join(moduleRoot, name));
    });
}

suite("Control lesson source structure", () => {
  test("uses domain datatypes for the substantial MPC request and result contracts", () => {
    for (const lesson of [
      "11-state-space-estimation-and-predictive-control/68-building-hvac-mpc",
      "14-integrated-capstones/83-building-hvac",
    ]) {
      const sourceRoot = path.join(curriculumRoot(), lesson, "src");
      const typesPath = path.join(sourceRoot, "MpcTypes.st");
      assert.ok(fs.existsSync(typesPath), `${lesson} must ship src/MpcTypes.st`);
      const types = fs.readFileSync(typesPath, "utf8");
      const controller = fs.readFileSync(
        path.join(sourceRoot, "FiniteGridMpcController.st"), "utf8"
      );
      const main = fs.readFileSync(path.join(sourceRoot, "Main.st"), "utf8");

      for (const typeName of ["BuildingMpcForecast", "BuildingMpcRequest", "BuildingMpcResult"])
        assert.match(types, new RegExp(`^TYPE ${typeName}\\s*:`, "m"));
      assert.match(controller, /Request\s*:\s*BuildingMpcRequest/);
      assert.match(controller, /Result\s*:\s*BuildingMpcResult/);
      assert.match(main, /MpcRequest\s*:\s*BuildingMpcRequest/);
      assert.match(main, /MpcResult\s*:\s*BuildingMpcResult/);
    }
  });

  test("keeps declarations out of Main and gives substantial stateful algorithms dedicated teaching components", () => {
    const lessons = lessonDirectories();
    assert.strictEqual(lessons.length, 84);

    for (const lessonRoot of lessons) {
      const main = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
      assert.doesNotMatch(
        main,
        /^(?:FUNCTION_BLOCK|FUNCTION|METHOD|CLASS|INTERFACE|TYPE)\b/m,
        `${path.relative(curriculumRoot(), lessonRoot)}/src/Main.st must contain only the main program and configuration`,
      );
    }

    const dedicatedComponents = [
      ["02-derivative-action-and-dynamic-response/25-servo-digital-realization", "DiscretePidController.st", "LessonDiscretePID"],
      ["09-industrial-control-structures/58-gravity-tank-scheduling", "GainScheduledTankController.st", "GainScheduledTankController"],
      ["11-state-space-estimation-and-predictive-control/68-building-hvac-mpc", "FiniteGridMpcController.st", "FiniteGridMpcController"],
      ["12-nonlinear-adaptive-and-robust-control/71-extruder-adaptive", "AdaptiveExtruderController.st", "AdaptiveExtruderController"],
      ["13-networked-fault-aware-and-diagnostic-control/73-networked-pumping", "NetworkedPumpSupervisor.st", "NetworkedPumpSupervisor"],
      ["13-networked-fault-aware-and-diagnostic-control/74-pumping-fault-aware", "FaultAwarePumpSupervisor.st", "FaultAwarePumpSupervisor"],
      ["14-integrated-capstones/78-servo", "ServoCandidate.st", "ServoCandidate"],
      ["14-integrated-capstones/79-reactor", "ReactorTrain.st", "ReactorTrain"],
      ["14-integrated-capstones/80-gravity-tank", "TankController.st", "TankController"],
      ["14-integrated-capstones/81-conveyor-oven", "OvenController.st", "OvenController"],
      ["14-integrated-capstones/82-distillation-column", "DistillationController.st", "DistillationController"],
      ["14-integrated-capstones/83-building-hvac", "FiniteGridMpcController.st", "FiniteGridMpcController"],
      ["14-integrated-capstones/84-independent", "CompressedAirController.st", "CompressedAirController"],
    ] as const;

    for (const [lesson, fileName, blockName] of dedicatedComponents) {
      const sourcePath = path.join(curriculumRoot(), lesson, "src", fileName);
      assert.ok(fs.existsSync(sourcePath), `${lesson} must ship src/${fileName}`);
      const source = fs.readFileSync(sourcePath, "utf8");
      assert.match(source, new RegExp(`^FUNCTION_BLOCK ${blockName}\\b`, "m"));
      assert.match(
        source,
        /^(?:\/\/|\(\*)[^\n]*(?:controller|supervisor|algorithm|candidate|train)/im,
        `${lesson}/src/${fileName} must introduce the teaching component and its purpose`,
      );
    }
  });
});
