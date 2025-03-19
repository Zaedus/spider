(function () {
  function send_color(color) {
    window.webkit.messageHandlers.themeColor.postMessage(color);
  }

  function send_none() {
    window.webkit.messageHandlers.themeColor.postMessage(null);
  }

  function update_and_listen(elm) {
    var observer = new MutationObserver((mutations) => {
      mutations.forEach((mutation) => {
        if (mutation.type === "attributes") {
          send_color(mutation.target.content);
        }
      });
    });

    send_color(elm.content);
    observer.observe(elm, { attributes: true });
  }

  function find_and_listen() {
    let elm = document.querySelector('meta[name="theme-color"]');
    if (elm) update_and_listen(elm);
    else send_none();
  }

  function listen_new_meta() {
    const observer = new MutationObserver((_) => {
      find_and_listen();
    });

    observer.observe(document.head, {
      childList: true,
      subtree: true,
    });
  }

  find_and_listen();
  listen_new_meta();
})();
