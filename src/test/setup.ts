import { setLang } from "@/i18n";
import { setPlatform } from "@/lib/platform";

// Unit tests assert the Italian reference texts unless a test switches language itself.
setLang("it");
// The platform guess comes from jsdom's user agent, which names the host OS: pin Windows
// (the base texts and the full section list) so results don't depend on the machine.
// macOS tests switch platform themselves.
setPlatform("windows");
