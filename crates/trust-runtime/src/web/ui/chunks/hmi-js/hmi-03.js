    });
  }
  const themeLabel = byId('themeLabel');
  if (themeLabel) {
    themeLabel.style.cursor = 'pointer';
    themeLabel.addEventListener('click', () => {
      cycleTheme();
    });
  }
  window.addEventListener('popstate', () => {
    syncStateFromRoute();
    ensureCurrentPage();
    renderSidebar();
    renderCurrentPage();
    void refreshActivePage({ forceValues: true });
    applyPresentationMode(state.presentationMode);
  });
  window.addEventListener('keydown', (event) => {
    if (event.defaultPrevented) {
      return;
    }
    if (event.key && event.key.toLowerCase() === 'g') {
      togglePresentationMode();
    }
  });
}


async function init() {
  syncStateFromRoute();
  initModeControls();
  try {
    const response = await apiControl('hmi.schema.get');
    if (!response.ok) {
      throw new Error(response.error || 'schema request failed');
    }
    renderSchema(response.result);
    await refreshDescriptorModel();
    await refreshActivePage({ forceValues: true });
    await refreshConnectorStatus();
    ensurePollingLoop();
    ensureConnectorPollingLoop();
    connectWebSocketTransport();
  } catch (error) {
    setEmptyMessage(`HMI unavailable: ${error}`);
    setConnection('disconnected');
    setFreshness(null);
  }
}

window.addEventListener('resize', () => {
  if (!state.schema) {
    return;
  }
  if (state.responsiveMode === 'auto') {
    applyResponsiveLayout();
  }
});

window.addEventListener('DOMContentLoaded', init);
