const ruleset = "lumina_rules";
const button = document.querySelector("#toggle");
const domainInput = document.querySelector("#domain");
const addButton = document.querySelector("#add");
const customList = document.querySelector("#custom");
const customRuleId = domain => { let hash = 7; for (const char of domain) hash = (hash * 31 + char.charCodeAt(0)) % 800000; return 1000 + hash; };
async function customDomains() { return (await chrome.storage.local.get({ customDomains: [] })).customDomains; }
async function renderCustom() { customList.innerHTML = ""; for (const domain of await customDomains()) { const item = document.createElement("li"); item.textContent = domain + " "; const remove = document.createElement("button"); remove.textContent = "quitar"; remove.style.cssText = "width:auto;padding:2px 5px;font-size:10px"; remove.onclick = async () => { const domains = (await customDomains()).filter(value => value !== domain); await chrome.storage.local.set({ customDomains: domains }); await chrome.declarativeNetRequest.updateDynamicRules({ removeRuleIds: [customRuleId(domain)] }); await renderCustom(); }; item.append(remove); customList.append(item); } }
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
addButton.addEventListener("click", async () => { const domain = domainInput.value.trim().toLowerCase().replace(/^https?:\/\//, "").replace(/\/.*$/, ""); if (!/^(?=.{1,253}$)([a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$/.test(domain)) { domainInput.setCustomValidity("Escribe un dominio válido"); domainInput.reportValidity(); return; } const domains = await customDomains(); if (domains.includes(domain)) return; await chrome.storage.local.set({ customDomains: [...domains, domain] }); await chrome.declarativeNetRequest.updateDynamicRules({ addRules: [{ id: customRuleId(domain), priority: 2, action: { type: "block" }, condition: { urlFilter: `||${domain}^`, resourceTypes: ["main_frame", "script", "image", "xmlhttprequest", "sub_frame", "media"] } }] }); domainInput.value = ""; await renderCustom(); });
void renderCustom();
