(() => {
  if (window !== window.top || location.origin !== 'https://tr-play.playall.in.th') return;
  const guard = '__OT_GUARD__';
  setInterval(() => window.ipc.postMessage(JSON.stringify({guard,kind:'target',canvas:!!document.querySelector('#unity-canvas'),instance:!!window.spinelInstance})),1000);
})();
