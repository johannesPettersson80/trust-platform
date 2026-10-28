// Run once on the reference device against a freshly built runtime. CPU browser results
// cannot establish this gate. Keep the report and screenshot even when a budget fails.
import fs from 'node:fs';
import {chromium} from '@playwright/test';
import {installPerformanceProbe,observePerformance,performanceReport} from './performance-observer.mjs';
const [url,output]=process.argv.slice(2);
if(!url||!output)throw new Error('Usage: node measure-reference.mjs URL OUTPUT.json');
const browser=await chromium.launch({executablePath:process.env.CHROMIUM_PATH||'/usr/bin/chromium',args:['--no-sandbox']});
try {
  const session=await browser.newBrowserCDPSession();
  const {gpu}=await session.send('SystemInfo.getInfo');
  const machine=fs.readFileSync('/proc/device-tree/model','utf8').replace(/\0/g,'').trim();
  const page=await browser.newPage({viewport:{width:1920,height:1080}});
  await page.addInitScript(installPerformanceProbe);
  const started=Date.now();
  await page.goto(url,{waitUntil:'commit'});
  await page.waitForFunction(()=>document.querySelectorAll('.hmi-page [data-state="confirmed"]').length>=300,null,{timeout:30000});
  await page.evaluate(()=>document.fonts.ready);
  const pageLoadMs=Date.now()-started;
  const movingSymbols=await page.locator('[data-motion-layer] [data-motion]').count();
  if(movingSymbols!==30)throw new Error(`Expected 30 moving symbols, found ${movingSymbols}`);
  const sample=await page.evaluate(observePerformance,20000);
  const report=performanceReport(sample,{measuredAt:new Date().toISOString(),sourceRevision:process.env.TRUST_VSCODE_TEST_SOURCE_COMMIT||'unrecorded',
    origin:'physical-pi-browser',runtimeOrigin:'remote-builder simulated IO',machine,browser:browser.version(),
    gpu:{devices:gpu.devices,features:gpu.featureStatus,renderer:gpu.auxAttributes?.glRenderer},viewport:{width:1920,height:1080},boundValues:300,movingSymbols,pageLoadMs});
  await page.screenshot({path:output.replace(/\.json$/,'.png')});
  fs.writeFileSync(output,JSON.stringify(report,null,2));
  console.log(JSON.stringify(report,null,2));
  if(report.referenceViolations.length)process.exitCode=1;
} finally {await browser.close();}
