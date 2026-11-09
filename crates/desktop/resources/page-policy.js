(() => {
  'use strict';
  if (window !== window.top || location.origin !== 'https://tr-play.playall.in.th') return;
  const guard = '__OT_GUARD__';
  const post = window.ipc.postMessage.bind(window.ipc);
  const notify = kind => post(JSON.stringify({guard,kind}));
  // Fullscreen stays inside the app's reserved pane. The host expands the pane.
  Element.prototype.requestFullscreen = function () {
    notify('fullscreen');
    return Promise.reject(new DOMException('Use the application game pane.', 'NotAllowedError'));
  };
})();
