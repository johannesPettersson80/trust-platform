// CPU/browser trend data, never a substitute for the Pi 5 or AM6442 hardware gates.
import { test, expect } from '@playwright/test';
import os from 'node:os';
import {installPerformanceProbe,observePerformance,performanceReport} from '../performance-observer.mjs';
test('records the complete 300-value and 30-motion workload without hiding budget violations',async({page,browser},info)=>{
  await page.addInitScript(installPerformanceProbe);
  const started=Date.now();
  await page.goto(`http://127.0.0.1:${process.env.HMI_SCREEN_PERFORMANCE_PORT||18483}/hmi/performance`);
  await page.waitForFunction(()=>document.querySelectorAll('.hmi-page [data-state="confirmed"]').length>=300);
  await page.evaluate(()=>document.fonts.ready);
  const pageLoadMs=Date.now()-started;
  await expect(page.locator('[data-motion-layer] [data-motion]')).toHaveCount(30);
  const sample=await page.evaluate(observePerformance,20000);
  const report=performanceReport(sample,{measuredAt:new Date().toISOString(),sourceRevision:process.env.GITHUB_SHA||process.env.TRUST_VSCODE_TEST_SOURCE_COMMIT||'unrecorded',
    origin:'builder-browser',hardwareQualification:false,machine:os.hostname(),architecture:os.arch(),browser:browser.version(),viewport:{width:1920,height:1080},boundValues:300,movingSymbols:30,pageLoadMs});
  await info.attach('hmi-performance-trend',{body:JSON.stringify(report,null,2),contentType:'application/json'});
  expect(sample.samples.length).toBeGreaterThan(20);expect(report.latencyMs.samples).toBeGreaterThan(20);
  // Workload validity is mandatory even on a CPU builder; hardware rendering budgets remain
  // separately reported trends. A slower source cannot qualify the requested 4 Hz workload.
  expect(report.updatesPerSecond, 'the fixture must deliver its declared 4 Hz input').toBeGreaterThanOrEqual(3.9);
});
