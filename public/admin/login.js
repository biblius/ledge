const loginForm = document.getElementById("login-form");

loginForm.addEventListener("submit", async (ev) => {
  ev.preventDefault();
  const pw = document.getElementById("password-input").value;

  const res = await fetch("/admin/login", {
    method: "post",
    body: JSON.stringify(pw),
    headers: {
      "content-type": "application/json",
    },
  });

  if (res.status === 200) {
    window.location.assign("/admin");
  }
});
