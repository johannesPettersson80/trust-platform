(function registerAdsRows(global) {
  function applyFilter(entries, filterValue) {
    const filter = String(filterValue || "").trim().toLowerCase();
    if (!filter) {
      return entries || [];
    }
    return (entries || []).filter((entry) =>
      [
        entry.name,
        entry.value,
        entry.valueType,
        entry.remoteSymbol,
        entry.connection,
        entry.access,
        entry.quality && entry.quality.state,
        entry.quality && entry.quality.detail,
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase()
        .includes(filter)
    );
  }

  function accessLabel(access) {
    if (access === "read_write") {
      return "Read/write";
    }
    return access === "write" ? "Write" : "Read-only";
  }

  function render(entries, problem, filterValue) {
    const wrapper = document.createElement("div");
    wrapper.className = "ads-rows";
    if (problem) {
      const notice = document.createElement("div");
      notice.className = "ads-contract-problem " + problem.kind;
      notice.setAttribute("role", "alert");
      const message = document.createElement("div");
      message.className = "ads-contract-problem-message";
      message.textContent = problem.message || "ADS values are unavailable.";
      notice.appendChild(message);
      const detail = document.createElement("div");
      detail.className = "ads-contract-problem-detail";
      detail.textContent =
        problem.detail ||
        "Reconnect or update truST before relying on ADS values.";
      notice.appendChild(detail);
      wrapper.appendChild(notice);
    }

    const filtered = applyFilter(entries, filterValue);
    if (filtered.length === 0) {
      const empty = document.createElement("div");
      empty.className = "empty";
      empty.textContent = problem
        ? "ADS values unavailable"
        : entries && entries.length > 0 && String(filterValue || "").trim()
        ? "No ADS matches"
        : "No imported ADS variables";
      wrapper.appendChild(empty);
      return wrapper;
    }

    const header = document.createElement("div");
    header.className = "ads-row-header";
    for (const label of ["Name", "Value", "Type", "Quality"]) {
      const cell = document.createElement("div");
      cell.textContent = label;
      header.appendChild(cell);
    }
    wrapper.appendChild(header);

    filtered.forEach((entry) => {
      const row = document.createElement("div");
      row.className = "ads-row quality-" + entry.quality.state;

      const nameCell = document.createElement("div");
      nameCell.className = "name";
      const name = document.createElement("div");
      name.textContent = entry.name;
      nameCell.appendChild(name);
      const remoteSymbol = document.createElement("div");
      remoteSymbol.className = "source-subtitle";
      remoteSymbol.textContent = entry.remoteSymbol;
      remoteSymbol.title = entry.remoteSymbol;
      nameCell.appendChild(remoteSymbol);
      const connection = document.createElement("div");
      connection.className = "source-subtitle";
      connection.textContent =
        entry.connection + " · " + accessLabel(entry.access);
      connection.title =
        "ADS connection " + entry.connection + " · " + accessLabel(entry.access);
      nameCell.appendChild(connection);

      const value = document.createElement("div");
      value.className = "value";
      value.textContent = entry.value;
      value.title = entry.value;

      const valueType = document.createElement("div");
      valueType.className = "type-cell";
      valueType.textContent = entry.valueType || "—";

      const qualityCell = document.createElement("div");
      qualityCell.className = "state-cell ads-quality";
      const quality = document.createElement("span");
      quality.className = "state-badge " + entry.quality.state;
      quality.textContent =
        entry.quality.state.slice(0, 1).toUpperCase() +
        entry.quality.state.slice(1);
      qualityCell.appendChild(quality);
      if (entry.quality.detail) {
        const detail = document.createElement("div");
        detail.className = "quality-detail";
        detail.textContent = entry.quality.detail;
        detail.title = entry.quality.detail;
        qualityCell.appendChild(detail);
      }

      row.appendChild(nameCell);
      row.appendChild(value);
      row.appendChild(valueType);
      row.appendChild(qualityCell);
      wrapper.appendChild(row);
    });
    return wrapper;
  }

  global.trustAdsRows = { render };
})(globalThis);
