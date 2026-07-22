(function () {
  var n = 0;
  var el = document.getElementById("count");
  var btn = document.getElementById("bump");
  var title = document.getElementById("title");
  if (title) {
    title.textContent = "竖屏预览 · JS OK";
  }
  if (btn && el) {
    btn.addEventListener("click", function () {
      n += 1;
      el.textContent = String(n);
    });
  }
})();
