import * as assert from "assert";
import { identifyPlant, type IdentificationSample } from "../../controlStudio/identification";

suite("Control Studio provisional step fitting", () => {
  test("retains the actuator ramp as part of the step instead of treating its second increment as restoration", () => {
    const samples: IdentificationSample[] = []; const inputs: number[] = [];
    let output = 50;
    for (let i=0;i<=300;i++) {
      const input = i<30 ? 20 : i<34 ? 20+(i-29)*2.5 : i<300 ? 30 : 27.5;
      inputs.push(input);
      samples.push({timeSeconds:i/10,input,output,quality:"good",inputUnit:"%",outputUnit:"°C"});
      const delayed = inputs[Math.max(0,i-5)];
      output = 20+1.5*delayed+(output-20-1.5*delayed)*Math.exp(-0.1/4);
    }
    let result: ReturnType<typeof identifyPlant> | undefined;
    assert.doesNotThrow(()=>{result=identifyPlant(samples);},"A ramp caused by the actuator rate limit must remain in the step record");
    assert.strictEqual(result!.partition,"single-run");
    assert.ok(result!.recommended.deadTimeSeconds!>0.2);
    assert.ok(Math.abs(result!.recommended.gain-1.5)<0.05);
  });
});
