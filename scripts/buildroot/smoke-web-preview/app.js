(function () {
  var n = 0;
  var el = document.getElementById("count");
  var btn = document.getElementById("bump");
  var title = document.getElementById("title");
  var apiEl = document.getElementById("api");
  var healthEl = document.getElementById("health");
  var tries = 0;
  if (title) {
    title.textContent = "竖屏预览 · JS OK";
  }
  if (btn && el) {
    btn.addEventListener("click", function () {
      n += 1;
      el.textContent = String(n);
    });
  }

  function apiBase() {
    if (typeof window.__MOONCODING_API_BASE__ === "string" && window.__MOONCODING_API_BASE__) {
      return window.__MOONCODING_API_BASE__;
    }
    return "";
  }

  function applyBase(base) {
    if (!base) {
      return;
    }
    window.__MOONCODING_API_BASE__ = base;
  }

  function probeHealth(base) {
    if (!healthEl) {
      return;
    }
    fetch(base + "/health")
      .then(function (r) {
        return r.json();
      })
      .then(function (j) {
        healthEl.textContent = j && j.ok ? "后端：OK" : "后端：异常";
      })
      .catch(function () {
        healthEl.textContent = "后端：未就绪";
        setTimeout(function () {
          probeHealth(base);
        }, 800);
      });
  }

  function probe() {
    tries += 1;
    var base = apiBase();
    if (apiEl) {
      apiEl.textContent = base
        ? ("API：" + base)
        : (tries < 25 ? "API：等待宿主注入…" : "API：未注入");
    }
    if (base) {
      if (healthEl) {
        healthEl.textContent = "后端：检测中…";
      }
      probeHealth(base);
      return;
    }
    // Fallback: read host lease written next to the project.
    fetch(".mooncoding/preview_backend.json")
      .then(function (r) {
        return r.ok ? r.json() : null;
      })
      .then(function (j) {
        if (j && j.api_base) {
          applyBase(j.api_base);
          probe();
          return;
        }
        if (tries < 25) {
          setTimeout(probe, 400);
        } else if (healthEl) {
          healthEl.textContent = "后端：无 API";
        }
      })
      .catch(function () {
        if (tries < 25) {
          setTimeout(probe, 400);
        } else if (healthEl) {
          healthEl.textContent = "后端：无 API";
        }
      });
  }

  probe();
})();
