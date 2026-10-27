// Runs against an installed VSIX in a clean profile. The development extension is only a
// harness; truST must resolve from the installation directory and use its packaged assets.
const fs=require('node:fs');
const os=require('node:os');
const path=require('node:path');
const {spawnSync}=require('node:child_process');
const repo=path.resolve(__dirname,'../..');
const electron=require(path.join(repo,'editors/vscode/node_modules/@vscode/test-electron'));
async function main(){
  const vsix=path.resolve(process.argv[2]||'');
  if(!process.argv[2]||!fs.statSync(vsix).isFile())throw new Error('Provide the produced VSIX.');
  const run=fs.mkdtempSync(path.join(os.tmpdir(),'trust-hmi-vsix-'));
  const project=path.join(run,'project'),harness=path.join(run,'harness'),extensions=path.join(run,'extensions'),profile=path.join(run,'profile');
  fs.mkdirSync(path.join(project,'src'),{recursive:true});fs.mkdirSync(path.join(project,'hmi/pages'),{recursive:true});fs.mkdirSync(harness);
  fs.writeFileSync(path.join(project,'src/main.st'),'PROGRAM Main\nVAR Level : REAL := 42.0; END_VAR\nEND_PROGRAM\n');
  fs.writeFileSync(path.join(project,'hmi/screen.toml'),'format = 1\nsize = [1280, 800]\ntheme = "dark"\nsymbols = "flat"\n');
  fs.writeFileSync(path.join(harness,'package.json'),JSON.stringify({name:'hmi-installed-smoke',publisher:'trust-test',version:'0.0.1',engines:{vscode:'^1.100.0'},main:'index.js',activationEvents:['*']}));
  fs.writeFileSync(path.join(harness,'index.js'),'exports.activate=()=>{};');
  const test=path.join(harness,'smoke.js');
  fs.writeFileSync(test,`
const vscode=require('vscode'),assert=require('assert'),fs=require('fs'),path=require('path');
exports.run=async()=>{
  const extension=vscode.extensions.getExtension('trust-platform.trust-lsp');assert.ok(extension,'installed truST extension');
  assert.ok(extension.extensionPath.startsWith(${JSON.stringify(extensions+path.sep)}),'must not load a development checkout');
  for(const file of ['media/fonts/InterVariable.woff2','media/hmiBuilderWebview.js','bin/trust-lsp'+(process.platform==='win32'?'.exe':'')]) assert.ok(fs.existsSync(path.join(extension.extensionPath,file)),file);
  await extension.activate();
  const deadline=Date.now()+60000;
  while(!(await vscode.commands.getCommands(true)).includes('trust-lsp.hmiFileParse')) {if(Date.now()>deadline)throw new Error('Packaged native server did not become available');await new Promise(r=>setTimeout(r,100));}
  const source=new vscode.CancellationTokenSource();
  async function call(name,input){const reply=await vscode.lm.invokeTool(name,{input:{contractVersion:1,input},toolInvocationToken:undefined},source.token);const result=JSON.parse(reply.content[0].value);assert.ok(result.ok,JSON.stringify(result));return {data:result.data,content:reply.content};}
  const context=await call('hmi_get_context',{});assert.strictEqual(context.data.compilation.status,'ready');
  const draft=await call('hmi_compose_page',{key:'installed-smoke',file:'hmi/pages/proposal.toml',title:'Level',pattern:'settings',items:[{id:'level',type:'value',region:'measurements',unit:'%',bind:{value:{path:'Main.Level',direction:'read'}}}]});
  const inspected=await call('hmi_inspect_page',{file:'hmi/pages/proposal.toml',draftId:draft.data.draftId,revision:draft.data.revision,ids:['level']});assert.strictEqual(inspected.data.origin,'draft');assert.deepStrictEqual(inspected.data.objects.map(object=>object.id),['level']);
  const image=await call('hmi_render_draft',{draftId:draft.data.draftId,revision:draft.data.revision});assert.ok(image.content.some(part=>part.mimeType==='image/png'),'client-visible packaged PNG');
  assert.ok(!fs.existsSync(path.join(${JSON.stringify(project)},'hmi/pages/proposal.toml')),'draft leaves source untouched');
  const connection=await vscode.commands.executeCommand('trust-lsp.hmiBuilder.connectAgent',{reveal:false});assert.ok(fs.existsSync(connection.connectionFile),'private bridge descriptor');assert.deepStrictEqual([connection.configuration.command,...connection.configuration.args],[path.join(extension.extensionPath,'bin','trust-lsp'+(process.platform==='win32'?'.exe':'')),'hmi-agent',connection.connectionFile],'the agent starts the packaged trust-lsp');
  await call('hmi_apply_draft',{draftId:draft.data.draftId,revision:draft.data.revision,action:'discard'});
  source.dispose();
};`);
  const executable=process.env.HMI_VSCODE_EXECUTABLE||await electron.downloadAndUnzipVSCode('1.140.0');
  const [cli,...args]=electron.resolveCliArgsFromVSCodeExecutablePath(executable);
  const installed=spawnSync(cli,[...args,'--no-sandbox','--extensions-dir',extensions,'--user-data-dir',profile,'--install-extension',vsix,'--force'],{stdio:'inherit'});
  if(installed.status!==0)throw new Error('VSIX installation failed');
  delete process.env.ST_LSP_TEST_SERVER;
  delete process.env.TRUST_LSP_SERVER;
  await electron.runTests({vscodeExecutablePath:executable,extensionDevelopmentPath:harness,extensionTestsPath:test,
    launchArgs:[project,'--no-sandbox','--disable-gpu','--disable-workspace-trust','--skip-welcome','--extensions-dir',extensions,'--user-data-dir',profile]});
  fs.rmSync(run,{recursive:true,force:true});
}
main().catch(error=>{console.error(error.message);process.exitCode=1;});
