const ruleset = "lumina_rules";
const button = document.querySelector("#toggle");
async function refresh() {
  const disabled = await chrome.declarativeNetRequest.getEnabledRulesets().then(ids => !ids.includes(ruleset));
  button.textContent = disabled ? "Activar protección" : "Desactivar protección";
  button.dataset.disabled = String(disabled);
}
button.addEventListener("click", async () => {
  const disabled = button.dataset.disabled === "true";
  await chrome.declarativeNetRequest.updateEnabledRulesets(disabled ? { enableRulesetIds: [ruleset], disableRulesetIds: [] } : { enableRulesetIds: [], disableRulesetIds: [ruleset] });
  await refresh();
});
void refresh();
