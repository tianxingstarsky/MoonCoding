(function () {
  var n = 0;
  var el = document.getElementById("count");
  var btn = document.getElementById("bump");
  var title = document.getElementById("title");
  var apiEl = document.getElementById("api");
  var healthEl = document.getElementById("health");
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

  function probe() {
    var base = apiBase();
    if (apiEl) {
      apiEl.textContent = base ? ("API：" + base) : "API：未注入（无 backend.py？）";
    }
    if (!base || !healthEl) {
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
        setTimeout(probe, 800);
      });
  }

  probe();
})();
