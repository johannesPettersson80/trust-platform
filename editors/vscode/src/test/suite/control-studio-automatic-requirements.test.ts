import * as assert from "assert";
import { automaticEngineerRequirements } from "../../controlStudio/automaticEngineerRequirements";
import { EngineerWorkflowSession } from "../../controlStudio/engineerWorkflowSession";
import type { GeneralModel } from "../../controlStudio/generalModel";
import { pidLoop, channel } from "./control-studio-pid-fixtures";

suite("Control Studio automatic test criteria", () => {
  const model = (tau = 4): GeneralModel => ({ kind: "transfer-function", numerator: [1.5], denominator: [tau, 1], delaySeconds: .5,
    inputs: [channel("u", "V")], outputs: [channel("y", "m")], operatingPoint: { inputs: [0], outputs: [0] },
    source: "provided", validation: "validated", evidence: ["declared-fixture"], revision: `model-${tau}` });
  async function tune(plant = model(), payload: Record<string, unknown> = {}, configure = (_loop: any) => {}) {
    const loop = pidLoop(); configure(loop);
    const session = new EngineerWorkflowSession(loop, () => plant, () => {});
    const messages: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "auto-criteria", projectId: loop.projectId,
        editorSessionId: "auto-criteria", loopId: loop.id, expectedLoopRevision: loop.revision,
        payload: { family: "PI", applicationMode: "manual", automaticRequirements: true, ...payload } },
      { projectId: loop.projectId, editorSessionId: "auto-criteria", loopId: loop.id, loopRevision: loop.revision }, async message => { messages.push(message); });
      const study = session.snapshotStudy();
      assert.ok(messages.some(message => message.status === "candidate"), JSON.stringify(messages.map(message => ({status: message.status,reason: message.reason}))));
      assert.ok(study?.task, "Tune must derive and retain the missing test settings");
      return { task: study!.task as any, messages, loop };
    } finally { session.dispose(); }
  }
  test("derives a bounded model-scaled task without explicit requirement fields", async () => {
    const fast = await tune(model(2)), slow = await tune(model(8));
    assert.ok(slow.task.settlingTimeSeconds > fast.task.settlingTimeSeconds);
    assert.ok(slow.task.durationSeconds > slow.task.settlingTimeSeconds && slow.task.durationSeconds <= 600);
    assert.ok(fast.task.settlingBand > 0 && fast.task.maximumError > fast.task.settlingBand);
    assert.ok(fast.task.automaticCriteria.fields.includes("maximumError"));
    assert.strictEqual(fast.task.automaticCriteria.modelRevision,"model-2");
    assert.deepStrictEqual(fast.messages.find(message => message.status === "candidate").task,fast.task);
  });
  test("keeps explicit process requirements and fills only missing settings", async () => {
    const result = await tune(model(), { maximumError: .4, settlingBand: .1, settlingTimeSeconds: 45, referenceChange: 2 });
    assert.strictEqual(result.task.maximumError,.4); assert.strictEqual(result.task.settlingBand,.1);
    assert.strictEqual(result.task.settlingTimeSeconds,45); assert.strictEqual(result.task.referenceChange,2);
    assert.ok(!result.task.automaticCriteria.fields.includes("maximumError"));
    assert.ok(result.task.durationSeconds > 45);
  });
  test("limits the automatically selected test step to actuator headroom", async () => {
    const result = await tune(model(), {}, loop => {loop.limits.outputMinimum=-.5;loop.limits.outputMaximum=.5;});
    assert.ok(result.task.referenceChange > 0 && result.task.referenceChange <= .075 + 1e-9);
  });
  test("recalculates generated values for a new model while preserving manual accuracy", async () => {
    const original = await tune(model(2), {maximumError:.4});
    const result = await tune(model(8), original.task);
    assert.strictEqual(result.task.maximumError,.4);
    assert.ok(result.task.settlingTimeSeconds > original.task.settlingTimeSeconds);
    assert.strictEqual(result.task.automaticCriteria.modelRevision,"model-8");
  });
  test("uses available downward movement when the actuator is at its upper limit", async () => {
    const plant=model(); plant.operatingPoint={inputs:[.5],outputs:[0]};
    const result=await tune(plant,{},loop=>{loop.limits.outputMinimum=-.5;loop.limits.outputMaximum=.5;});
    assert.ok(result.task.referenceChange<0,'Choose a feasible direction instead of requesting unavailable upward actuation');
  });
  test("derives each explicitly mapped output in its own units and actuator bounds", () => {
    const loop=pidLoop();
    const signal=(id:string,role:string,unit:string,address:string)=>({id,role,channel:id,symbol:`Main.${id}`,address,iecType:'LREAL',direction:role==='SP'?'write':'read',unit,range:{minimum:-20,maximum:20}});
    loop.signals.additional=[signal('mv2','MV','V','%ML32'),signal('pv2','PV','°C','%ML40'),signal('sp2','SP','°C','%ML48')];
    loop.channels={inputs:[{modelChannelId:'u1',signalId:'manipulatedVariable',actuator:{kind:'continuous',minimum:-20,maximum:20}},
      {modelChannelId:'u2',signalId:'mv2',actuator:{kind:'continuous',minimum:-.1,maximum:.1}}],
      outputs:[{modelChannelId:'y1',signalId:'processVariable',setpointSignalId:'setpoint'},
        {modelChannelId:'y2',signalId:'pv2',setpointSignalId:'sp2'}]};
    const plant:GeneralModel={...model(),kind:'continuous-state-space',inputs:[channel('u1','V'),channel('u2','V')],
      outputs:[channel('y1','m'),channel('y2','°C')],states:[channel('x1','m'),channel('x2','°C')],
      a:[[-.25,0],[0,-.5]],b:[[.25,0],[0,1]],c:[[1,0],[0,1]],d:[[0,0],[0,0]],operatingPoint:{inputs:[0,0],outputs:[0,0]}};
    const task=automaticEngineerRequirements(loop,plant,{},[]);
    assert.ok(task.channelRequirements!.y1.referenceChange>task.channelRequirements!.y2.referenceChange);
    assert.ok(task.channelRequirements!.y2.referenceChange<=.020000001);
    assert.ok(task.automaticCriteria!.fields.includes('maximumError:y2'));
    assert.strictEqual(task.channelRequirements!.y1.maximumError,task.maximumError);
  });

});
