## DevTrack 0.1.5

### Novità
- **Codice aperto:** il codice di DevTrack è pubblico, con licenza MIT, su [github.com/brainupstudio/devtrack](https://github.com/brainupstudio/devtrack), con una guida all'uso.
- **Importazione da un altro strumento di tracciamento:** accanto al token API ora si indica anche l'indirizzo del servizio da cui leggere le voci. Nessuno dei due viene salvato.
- **Formati della durata** con nomi più chiari: "Ore, minuti e secondi", "Minuti e secondi", "Ore decimali".

### Download
| Sistema | File |
|---|---|
| macOS 11 o successivo (Apple silicon e Intel) | `DevTrack-0.1.5-macOS.dmg` |
| Windows 10/11 (x64) | `DevTrack-0.1.5-Windows-x64.zip` |
| Linux (x64) | `DevTrack-0.1.5-Linux-x64.tar.gz` |

### Aggiornare
Dalla 0.1.3, su macOS, DevTrack si aggiorna da solo. Dalle versioni precedenti installa questa al posto di quella che hai, con il comando qui sotto o dal file scaricato: le sessioni, i progetti e le impostazioni restano dove sono.

### Installare dal Terminale, senza avvisi
Un comando scarica l'ultima versione, controlla che sia quella pubblicata e la installa; serve anche per aggiornare. Il sistema non mostra l'avviso sulle app non firmate.
- **macOS e Linux**, nel Terminale: `curl -fsSL https://brainupstudio.github.io/devtrack.site/install.sh | sh`
- **Windows**, in PowerShell: `irm https://brainupstudio.github.io/devtrack.site/install.ps1 | iex`

### Primo avvio dal file scaricato
Questa versione non è ancora firmata dagli editori dei sistemi operativi, quindi scaricata dal browser mostra un avviso la prima volta.
- **macOS:** apri il DMG e trascina DevTrack in Applicazioni (sostituisci quella che c'è). Al primo avvio macOS dice che non può verificarla: premi Fine, apri Impostazioni di Sistema › Privacy e sicurezza e, in fondo, premi "Apri comunque". Serve una volta sola.
- **Windows:** estrai lo zip e avvia `DevTrack.exe`. Se compare "Windows ha protetto il PC", scegli "Ulteriori informazioni" › "Esegui comunque".
- **Linux:** estrai l'archivio ed esegui `./install.sh` per aggiungere DevTrack al menu delle applicazioni, oppure avvia direttamente `./devtrack`.

### Limiti noti
- L'icona nella barra dei menu e l'apertura all'avvio ci sono solo su macOS.
- Le versioni per Windows e Linux sono versioni di anteprima: non sono ancora state provate su un PC con quei sistemi. Se trovi un problema, segnalacelo.

## DevTrack 0.1.4

- **Tema scuro più morbido:** niente più nero assoluto. Il fondo è un grigio molto scuro, appena caldo, e barra laterale, pannelli e campi si staccano a livelli; i colori di stato (ambra, verde, rosso) restano gli stessi e il testo resta ben leggibile. Su macOS anche il bordo della finestra è quello delle app scure.
- **Liste più eleganti:** quando una lista continua sopra o sotto, le righe sfumano nel fondo invece di essere tagliate di netto.
- **Rendicontazione:** sotto ogni riga c'è anche il cliente accanto al progetto; nella riga per fogli di calcolo le colonne che resteranno vuote si vedono, e il codice mancante non è più ripetuto in un riquadro.

## DevTrack 0.1.3

- **Aggiornamento automatico su macOS:** quando esce una versione nuova, DevTrack la scarica da sola, ne controlla l'impronta e la installa quando la chiudi (⌘Q o "Esci" dalla barra dei menu). Se vuoi subito, "Riavvia per aggiornare" nella barra laterale oppure "Riavvia e aggiorna" in Impostazioni › Generale. La nuova versione si apre senza l'avviso di macOS e i tuoi dati restano dove sono. Su Windows e Linux resta il collegamento alla pagina di download.
- **Controllo degli aggiornamenti più affidabile:** non usa più l'interfaccia di GitHub con il suo limite di richieste, che in un ufficio con molte persone sulla stessa rete poteva esaurirsi.

## DevTrack 0.1.2

- **Timer:** il campo attività prende tutta la riga; sotto, progetto e cliente sono due parole sottolineate che aprono gli stessi menu, con la ricerca, della finestra della sessione. Puoi scegliere prima il cliente: i progetti si restringono ai suoi.
- **Sessioni:** nelle finestre "Modifica sessione" e "Aggiungi sessione" l'ordine è Attività, Progetto, Cliente; il cliente si cerca e, se manca, si crea scrivendone il nome.
- **Menu con la ricerca:** resta ferma solo la ricerca e tutto il resto scorre insieme, alto quanto lo spazio disponibile; prima, dopo una ricerca, l'elenco poteva ridursi a una riga.
- **Esportazione CSV:** si trova in Impostazioni › Dati, con la scelta del periodo (questa settimana, la scorsa, questo mese, il mese scorso).
- **Rendicontazione:** i dettagli di una riga sono divisi in tre parti (sessioni, calcolo, riga per fogli di calcolo) e spiegano a cosa serve la riga da incollare in Excel o in un altro foglio di calcolo.

## DevTrack 0.1.1

- **Aggiornamenti:** all'apertura, una volta al giorno, DevTrack controlla se è uscita una versione nuova e lo segnala nella barra laterale, con il collegamento alla pagina di download. In Impostazioni › Generale c'è "Controlla ora", e il controllo automatico si può spegnere. Viene chiesto solo il numero dell'ultima versione: nessun dato tuo o del tuo lavoro.
- **Sessioni:** nelle finestre "Modifica sessione" e "Aggiungi sessione" c'è il campo Cliente. Segue il progetto scelto; scelto per primo, mostra solo i progetti di quel cliente.
- **Windows:** l'icona di DevTrack è dentro `DevTrack.exe`, quindi la mostrano anche Esplora file, il Desktop e i collegamenti; in Proprietà › Dettagli compaiono nome e versione.

## DevTrack 0.1.0 — primo rilascio

Tracciamento del tempo personale, con le attività di Azure DevOps in sola lettura e i valori pronti per ogni sistema in cui riporti le ore.

- **Oggi:** timer con pausa e ripresa, sessioni passate, elenco, calendario settimanale e griglia Timesheet; il tempo convertito per ogni piattaforma accanto a quello reale.
- **Lavoro:** progetti, clienti e l'elenco di tutte le attività, con filtri per cliente, progetto (anche "senza"), origine e stato; colori, attività locali o importate da Azure DevOps, ore previste (la stima di DevOps per le attività importate, la somma delle attività per il progetto).
- **Rendicontazione:** i valori da inserire in ogni piattaforma, nel suo formato e con il suo arrotondamento, con codici per progetto ed eccezioni per attività.
- **Statistiche:** tempo per giorno, progetto e attività.
- **Importazione da un altro strumento di tracciamento:** le sessioni degli ultimi 7, 30 o 90 giorni, con anteprima e senza doppioni.
- **Impostazioni:** piattaforme, giorni lavorativi, inattività e promemoria, sincronizzazione tra dispositivi tramite una cartella condivisa, blocco dei giorni già rendicontati.
- **macOS:** icona nella barra dei menu con il tempo della sessione in corso; chiudendo la finestra DevTrack resta attivo; apertura all'avvio del Mac.
- Interfaccia in italiano e in inglese, tema scuro e chiaro.
