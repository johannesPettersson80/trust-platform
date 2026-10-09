// Browser-only functions passed directly to Playwright. Completion means every value card
// matches one received frame; first-card latency would conceal deferred painting.
export function installPerformanceProbe() {
  const log=window.__hmiPerformance={samples:[],long:[],latest:{},current:null};
  const Native=window.WebSocket;
  window.WebSocket=class extends Native {
    constructor(...args){
      super(...args);
      this.addEventListener('message',event=>{
        let message;try{message=JSON.parse(String(event.data));}catch{return;}
        let count=0;
        for(const [name,wire] of Object.entries(message.values||{})){
          const match=/^Main\.Values\[(\d+)\]$/i.exec(name);
          if(match&&typeof wire.value==='number'){
            log.latest[`v${Number(match[1])}`]=wire.value.toFixed(1);count++;
          }
        }
        if(count){log.current={received:performance.now(),expected:{...log.latest},completed:null};log.samples.push(log.current);}
      });
    }
  };
  new PerformanceObserver(list=>{for(const entry of list.getEntries())log.long.push({start:entry.startTime,duration:entry.duration});}).observe({type:'longtask',buffered:true});
}
export async function observePerformance(windowMs) {
  const log=window.__hmiPerformance,frames=[],nodes=new Map();
  for(let index=1;index<=270;index++)nodes.set(`v${index}`,document.querySelector(`[data-object="v${index}"] tspan`));
  const completed=()=>{
    const sample=log.current;
    if(!sample||sample.completed!==null||Object.keys(sample.expected).length!==270)return;
    for(const [id,text] of Object.entries(sample.expected)){
      let node=nodes.get(id);
      if(!node?.isConnected){node=document.querySelector(`[data-object="${id}"] tspan`);nodes.set(id,node);}
      if(node?.textContent!==text)return;
    }
    sample.completed=performance.now();
  };
  const observer=new MutationObserver(completed);
  observer.observe(document.querySelector('.hmi-surface'),{subtree:true,childList:true,characterData:true,attributes:true});
  log.samples=[];
  const start=performance.now();
  await new Promise(resolve=>{const tick=time=>{frames.push(time);completed();if(performance.now()-start<windowMs)requestAnimationFrame(tick);else resolve();};requestAnimationFrame(tick);});
  const end=performance.now();
  // Give the last in-window frame its full latency budget instead of counting window cutoff
  // as a dropped update. Later arrivals and extra RAFs do not inflate the measured update/FPS rate.
  await new Promise(resolve=>{const tick=time=>{frames.push(time);completed();if(performance.now()-end<250)requestAnimationFrame(tick);else resolve();};requestAnimationFrame(tick);});
  observer.disconnect();
  return {start,end,samples:log.samples.filter(item=>item.received<end).map(({received,completed})=>({received,completed})),frames,long:log.long.filter(item=>item.start>=start&&item.start<end)};
}
export function performanceReport(sample,metadata) {
  const percentile=(sorted,p)=>sorted.length?sorted[Math.min(sorted.length-1,Math.ceil(sorted.length*p)-1)]:null;
  const latencies=[];
  for(const entry of sample.samples){
    const painted=entry.completed===null?undefined:sample.frames.find(time=>time>=entry.completed);
    if(painted!==undefined)latencies.push(painted-entry.received);
  }
  latencies.sort((a,b)=>a-b);
  const seconds=(sample.end-sample.start)/1000;
  const report={...metadata,durationSeconds:seconds,updatesPerSecond:sample.samples.length/seconds,framesPerSecond:sample.frames.filter(time=>time>=sample.start&&time<=sample.end).length/seconds,
    latencyMs:{samples:latencies.length,p50:percentile(latencies,.5),p95:percentile(latencies,.95),p99:percentile(latencies,.99),max:latencies.at(-1)??null},
    incompleteFrames:sample.samples.filter(entry=>entry.completed===null).length,longTasks:sample.long,over50ms:sample.long.filter(item=>item.duration>50).length,
    referenceBudgets:{pageLoadMs:2000,framesPerSecond:30,p95:100,p99:250,longTaskMs:50,updatesPerSecond:4}};
  report.referenceViolations=[...(report.pageLoadMs>2000?['page-load']:[]),...(report.updatesPerSecond<3.9?['update-rate']:[]),
    ...(report.framesPerSecond<30?['frame-rate']:[]),...(report.latencyMs.p95===null||report.latencyMs.p95>100?['latency-p95']:[]),
    ...(report.latencyMs.p99===null||report.latencyMs.p99>250?['latency-p99']:[]),...(report.over50ms>0?['long-tasks']:[]),
    ...(report.incompleteFrames>0?['incomplete-frames']:[])];
  return report;
}
