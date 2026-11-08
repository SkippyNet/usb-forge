# Secure Boot and your USB Forge drive

USB Forge tells you when **Secure Boot is turned on** on the PC you're using. Some tools on a
USB Forge drive only start with Secure Boot **turned off** in the PC's firmware (UEFI/BIOS)
settings.

> **Caveat:** Firmware menus differ between manufacturers, models, BIOS versions and regions,
> and vendors change them over time. The steps below are a general guide and **may not match
> your exact machine**. If something doesn't match, search for "*your model* disable Secure Boot"
> or check the manufacturer's support site.

## Before you change anything

1. **BitLocker / device encryption (Windows):** changing Secure Boot can trigger a BitLocker
   recovery prompt. Make sure you have your recovery key first (Microsoft account →
   *Devices* → *BitLocker recovery keys*, or `manage-bde -protectors -get C:` from an admin prompt).
2. **Firmware passwords:** some business PCs need a supervisor/admin BIOS password before
   Secure Boot can be changed. Ask your IT department if the PC is managed.
3. **Turn it back on afterwards** if you only needed the USB drive temporarily. Windows 11 and
   some anti-cheat software expect Secure Boot to be on.

## Getting into the firmware settings

**From Windows 10/11 (works on most PCs):** Settings → System → Recovery → *Advanced startup* →
**Restart now** → Troubleshoot → Advanced options → **UEFI Firmware Settings** → Restart.

**From power-on:** press the setup key repeatedly right after switching on (see the table).

## Common vendors

| Vendor | Setup key | Where Secure Boot usually is | Notes |
|---|---|---|---|
| **Dell** | F2 | *Boot Configuration* (newer) or *Secure Boot → Secure Boot Enable* | One-time boot menu: F12 |
| **HP** (consumer and business) | Esc, then F10 | *Advanced* or *Security → Secure Boot Configuration*; set *Secure Boot Control* to Disabled | Confirms the change with a 4-digit code on the next boot. Boot menu: Esc → F9 |
| **Lenovo ThinkPad** | F1 (or Enter → F1) | *Security → Secure Boot* | Boot menu: F12 |
| **Lenovo IdeaPad / Yoga / Legion** | F2 (or the Novo button) | *Security → Secure Boot* | Some models need *Fn+F2*. Boot menu: F12 |
| **ASUS** desktop boards (UEFI) | Del or F2 | *Advanced Mode (F7) → Boot → Secure Boot → OS Type* → **Other OS** | Some boards: *Key Management → Clear Secure Boot keys* |
| **ASUS** laptops | F2 | *Security → Secure Boot Control* → Disabled | Boot menu: Esc |
| **Acer** | F2 | *Security* → set a supervisor password first, then *Boot → Secure Boot* | The option stays greyed out until a supervisor password is set. Boot menu: F12 (may need enabling) |
| **MSI** | Del (desktops) / F2 (laptops) | *Settings → Advanced → Windows OS Configuration → Secure Boot* | Boot menu: F11 |
| **Gigabyte / AORUS** | Del | *Boot → Secure Boot* (or set *CSM Support* / *Windows 8/10 Features* to Other OS on older boards) | Boot menu: F12 |
| **ASRock** | F2 or Del | *Security → Secure Boot* | Boot menu: F11 |
| **Microsoft Surface** | Hold Volume Up while pressing Power | *Security → Secure Boot* → **Disabled** | A red bar at boot is normal while it's off |
| **Framework** | F2 | *Administer Secure Boot → Enforce Secure Boot* → Disabled | Boot menu: F12 |
| **Toshiba / Dynabook** | F2 | *Security → Secure Boot* | Boot menu: F12 |
| **Samsung** | F2 | *Boot → Secure Boot Control* | May also need *OS Mode Selection* → UEFI and CSM |
| **Apple Mac (Intel, T2 chip)** | Hold Cmd+R at startup → Utilities → *Startup Security Utility* | Set *No Security* and *Allow booting from external media* | Apple Silicon Macs can't boot PC USB drives |
| **Virtual machines** (Hyper-V, VMware, VirtualBox) | – | VM settings → Firmware/Security → Secure Boot | Hyper-V Gen 2: VM Settings → Security |

After saving (usually **F10**), choose the USB drive from the one-time boot menu.

## Turning Secure Boot back on

Go back to the same menu and enable it again. If it refuses, look for **Restore Factory Keys**
or **Install default Secure Boot keys** in the same section, then enable it.
