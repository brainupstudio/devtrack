# Rilasciare DevTrack

Lo sviluppo e le release partono da questo repository, `brainupstudio/devtrack` (pubblico). Ogni
commit qui è visibile a tutti: niente dati personali, percorsi, token, note di lavoro interne o
riferimenti ad altre app. Le release si pubblicano poi anche su `brainupstudio/devtrack.site`
(passo 5), da cui le app installate si aggiornano.

## I pacchetti

| Sistema | Pacchetto | Come si costruisce |
|---|---|---|
| macOS 11+, Apple silicon e Intel | `DevTrack-<versione>-macOS.dmg` (un'app universale) | `scripts/bundle-macos.sh --universal --dmg`, su un Mac o in CI |
| Windows 10/11 x64 | `DevTrack-<versione>-Windows-x64.zip` (`DevTrack.exe` + istruzioni) | GitHub Actions, `windows-latest` |
| Linux x64 | `DevTrack-<versione>-Linux-x64.tar.gz` (programma, icona, voce di menu, `install.sh`) | GitHub Actions, `ubuntu-22.04` |

Su Windows `build.rs` incorpora nel `.exe` l'icona (`assets/icons/app-icon.ico`, da 16 a 256 px), il nome e la versione; il workflow controlla che ci siano. Windows e Linux si costruiscono solo in CI: da un Mac non si possono compilare, perché la libreria crittografica (`ring`) richiede il compilatore C di quei sistemi.

## Passi

1. Aggiorna `version` in `Cargo.toml` e le note in `CHANGELOG.md` (diventano il testo del rilascio).
2. Prova i pacchetti senza pubblicare nulla: GitHub › Actions › **Release** › *Run workflow*. I tre file restano come artefatti dell'esecuzione.
3. Crea e invia il tag:
   ```sh
   git tag v<versione>
   git push origin v<versione>
   ```
   Il workflow costruisce i tre pacchetti e li allega a una **bozza** di rilascio "DevTrack v<versione>".
   Insieme ai pacchetti pubblica `SHA256SUMS.txt`, le loro impronte.
4. Controlla la bozza su GitHub › Releases, scarica e prova almeno un pacchetto per sistema, poi pubblicala. Verifica anche che i programmi non contengano dati personali: `strings <programma> | grep -c "/Users/"` deve trovare solo `/Users/runner/`.
5. Copia i pacchetti e `SHA256SUMS.txt` in una release `v<versione>` del sito (senza il DMG e le impronte lì, l'aggiornamento automatico su macOS non parte), `brainupstudio/devtrack.site`, segnata come ultima, e aggiorna la versione in `js/config.js` del sito. È da lì che si scarica: la pagina di download, il controllo aggiornamenti dell'app e gli script `install.sh` / `install.ps1` leggono l'ultima release del sito, e gli script si fermano se mancano le impronte.
   ```sh
   gh release download v<versione> -D /tmp/devtrack-<versione>
   gh release create v<versione> -R brainupstudio/devtrack.site --title "DevTrack v<versione>" --notes-file CHANGELOG.md --latest /tmp/devtrack-<versione>/*
   ```

Sul Mac, per provare l'app universale in locale:
```sh
scripts/bundle-macos.sh --universal --dmg
```
Il DMG finisce in `dist/` (ignorata da Git).

## Firma

Su macOS il workflow firma l'app con un certificato autoprodotto, sempre lo stesso (segreti `MACOS_SIGN_P12`, il file .p12 in base64, e `MACOS_SIGN_P12_PASSWORD`). Così ogni versione ha la stessa identità di firma. Non basta però al portachiavi: senza un Team ID di Apple, macOS lega l'accesso anche all'impronta del singolo eseguibile (partizione `cdhash:`), quindi dopo ogni aggiornamento chiede una volta la password. Non toglie nemmeno l'avviso di Gatekeeper. Per entrambe le cose serve il certificato Developer ID descritto sotto. Senza i segreti, per esempio in un fork, l'app viene firmata ad hoc.

Senza una firma riconosciuta i sistemi mostrano un avviso al primo avvio dei pacchetti scaricati dal browser; le istruzioni per superarlo sono in `CHANGELOG.md`. Gli script di installazione del sito (`curl … | sh`, `irm … | iex`) scaricano senza il segno "da internet", quindi l'avviso non compare: è la strada gratuita finché l'app non è firmata.

- **macOS:** serve un certificato *Developer ID Application* (programma Apple Developer, 99 USD/anno; i certificati *Apple Development* presenti su questo Mac non bastano). Con il certificato nel portachiavi:
  ```sh
  DEVTRACK_SIGN_ID="Developer ID Application: <nome> (<team>)" scripts/bundle-macos.sh --universal --dmg
  xcrun notarytool submit dist/DevTrack-<versione>-macOS.dmg --keychain-profile <profilo> --wait
  xcrun stapler staple dist/DevTrack-<versione>-macOS.dmg
  ```
  In CI andrebbero aggiunti il certificato e le credenziali di notarizzazione come segreti del repository.
- **Windows:** serve un certificato di firma del codice (Authenticode); senza, SmartScreen mostra "Windows ha protetto il PC".
- **Linux:** non serve.
