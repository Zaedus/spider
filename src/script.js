(function() {
  function update() {
    document.querySelector('meta[name="theme-color"]').content = "#000000";
  }
  function listen(elm) {
    var observer = new MutationObserver((mutations) => {
      mutations.forEach((mutation) => {
        if (mutation.type === "attributes") {
          if (elm.content == "#000000") return;
          update();
        }
      });
    });

    observer.observe(elm, { attributes: true });
  }

  function find_and_listen() {
    let elm = document.querySelector('meta[name="theme-color"]');
    if (elm) {
      listen(elm);
    } else { 
      let elm = document.createElement("meta");
      elm.name = "theme-color";
      document.head.appendChild(elm);
    }
    update();
  }

  function listen_new_meta() {
    const observer = new MutationObserver(_ => {
      find_and_listen();
    });

    observer.observe(document.head, {
      childList: true,
      subtree: true
    });
  }

  find_and_listen();
  listen_new_meta();
})();
