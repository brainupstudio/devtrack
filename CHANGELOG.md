## DevTrack 0.1.18

### Novità
- **Tag in un elenco unico:** i tag sono un solo elenco, sincronizzato tra i tuoi dispositivi, senza doppioni per maiuscole o minuscole. I tag che avevi scritto a mano entrano nell'elenco da soli. In Lavoro › Tag puoi rinominarli, unirli ed eliminarli; le sessioni si aggiornano, e c'è Annulla.
- **Scegliere i tag:** nel timer (link Tag, oppure scrivendo `#parola` in fondo all'attività), nella modifica di una sessione, in Aggiungi sessione e nella finestra compatta c'è un menu con ricerca e caselle, da cui puoi anche creare un tag nuovo. La ricerca trova anche le varianti scritte in modo diverso ("code-review" trova "Code review") e non propone di crearne una copia.
- **Filtrare per tag:** in Oggi fai clic su un `#tag` di una sessione, oppure scegli il tag dal menu a destra del totale: totale, barra dei progetti ed elenco mostrano solo quelle sessioni, e nel calendario le altre restano visibili ma attenuate. Il filtro c'è anche nelle Statistiche (con "Senza tag" e una sezione Tag su cui puoi cliccare) e in Lavoro › Attività.
- **Tag su più sessioni insieme:** in Oggi spunta le sessioni (la casella al posto del pallino, Seleziona nel menu ··· della riga, Spazio o ⌘-clic) e usa Aggiungi tag o Togli tag nella barra che compare in cima all'elenco. Le sessioni di un periodo bloccato restano com'erano; c'è Annulla.
- **Tag detti una volta:** su un'attività, su un valore della Rendicontazione e su un gruppo di Per attività i tag compaiono una volta sola; sulle singole sessioni restano solo quelli che le distinguono. Passando sopra "+N" vedi tutti i tag.
- **Per attività più chiaro:** aprendo un gruppo, le sue sessioni sono rientrate e unite al gruppo da una linea, con la propria nota, i tag e gli orari. Il gruppo dice quante sessioni contiene, invece di un orario dalla prima all'ultima che sembrava un blocco unico.
- **Assistenti AI:** possono elencare i tag, filtrare le sessioni senza tag e preparare un report per tag.
- **Finestra compatta:** a timer fermo ha la stessa forma di quando è in corso, con progetto, cliente e Tag come link; una nuova vista Calendario mostra un giorno alla volta; le righe mostrano anche cliente e tag e hanno il pulsante delle conversioni ⇄.
- **Calendario:** i blocchi si allungano trascinando sia il bordo superiore sia quello inferiore.

### Correzioni
- **Minuti interi:** quando cambi l'orario di una sessione (frecce, testo, durata o trascinamento) inizio e fine perdono i secondi, così la durata è tonda.
- Il pulsante di copia della giornata è stato tolto.
- I suggerimenti al passaggio del mouse vanno a capo dentro la finestra, anche nella compatta.
- In Statistiche "No project" ora è "Nessun progetto".

### Download
| Sistema | File |
|---|---|
| macOS 11 o successivo (Apple silicon e Intel) | `DevTrack-0.1.18-macOS.dmg` |
| Windows 10/11 (x64) | `DevTrack-0.1.18-Windows-x64.zip` |
| Linux (x64) | `DevTrack-0.1.18-Linux-x64.tar.gz` |

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

## DevTrack 0.1.17

### Novità
- **Ore per attività:** in Oggi una nuova vista, Per attività, mostra una riga per ogni attività con il totale del giorno, dalla prima partenza all'ultima fine. Apri la riga per vederne le sessioni. Nella finestra compatta lo stesso raggruppamento si attiva con il pulsante accanto ad "Aggiungi sessione".
- **Copia della giornata:** il pulsante di copia in fondo a ogni giorno copia i totali per attività, con il valore per ogni piattaforma e la riga del totale, pronti da incollare in un foglio di calcolo.
- **Barra di Oggi più compatta:** le quattro viste (Elenco, Per attività, Calendario, Timesheet) sono icone accanto a Giorno, Settimana e Mese. Sotto resta il totale, con lo stato delle sessioni scritto a parole.

### Correzioni
- **Modifica di una sessione:** scegliere il progetto dopo l'attività non azzera più il resto. L'attività resta se appartiene al progetto; se appartiene a un altro, viene tolta e un avviso dice perché. La ricerca dei progetti trova anche quelli degli altri clienti.
- **Barra colorata dei progetti:** l'elenco dei progetti si apre sotto il segmento su cui sei, con il suo progetto in evidenza, e la barra si raggiunge più facilmente con il mouse.

### Download
| Sistema | File |
|---|---|
| macOS 11 o successivo (Apple silicon e Intel) | `DevTrack-0.1.17-macOS.dmg` |
| Windows 10/11 (x64) | `DevTrack-0.1.17-Windows-x64.zip` |
| Linux (x64) | `DevTrack-0.1.17-Linux-x64.tar.gz` |

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

## DevTrack 0.1.16

### Novità
- **Assistenti AI:** Claude e gli altri assistenti compatibili con MCP possono leggere il tempo che registri e prepararti risposte e report per i PM, per esempio "Quante ore ho fatto la settimana scorsa per cliente?" o "Prepara il report della settimana per il PM di Portale clienti". Le cifre le calcola DevTrack dalle tue sessioni, con le stesse regole dell'app; l'assistente scrive il testo intorno. Le durate sono scritte come "4 h 18 min", e i report contengono tabelle per attività, per giorno e per progetto.
- **Impostazioni › Assistenti AI:** prima trovi cosa può leggere un assistente, poi l'interruttore per attivarlo, spento finché non lo accendi e valido solo su quel dispositivo. Una riga verifica da sola che tutto funzioni. Claude Desktop si collega con un clic, dopo una conferma e una copia del suo file di configurazione. Per Claude Code e gli altri assistenti c'è il comando da copiare, e ci sono anche esempi di domande da fare.
- **Azure DevOps negli assistenti:** con un secondo interruttore, l'assistente legge anche i work item assegnati a te e, quando serve, ne legge uno dal vivo: stato, Completed e Remaining Work, ultimi cambiamenti. I report mostrano allora l'avanzamento rispetto alle stime, con le ore di DevTrack accanto a quelle dichiarate su DevOps.
- **Report pronti e anonimizzati:** in Claude Desktop ci sono tre report pronti (settimanale per il PM, stato di un progetto, riepilogo per cliente). Per un report da mandare fuori dal team puoi chiederlo anonimizzato: i clienti diventano "Client A", "Client B" e le note restano fuori.

### Da sapere
- L'assistente legge e basta: non può cambiare nulla, né in DevTrack né su Azure DevOps, e non vede mai i token. Quello che legge però va al servizio dell'assistente che usi (Anthropic per Claude, OpenAI per ChatGPT e Codex).

### Download
| Sistema | File |
|---|---|
| macOS 11 o successivo (Apple silicon e Intel) | `DevTrack-0.1.16-macOS.dmg` |
| Windows 10/11 (x64) | `DevTrack-0.1.16-Windows-x64.zip` |
| Linux (x64) | `DevTrack-0.1.16-Linux-x64.tar.gz` |

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

## DevTrack 0.1.15

### Correzioni
- **Icona nel Dock con DevTrack aperto:** mentre l'app è aperta, il Dock mostrava una "e" al posto del logo. Ora l'icona è la stessa con l'app aperta e chiusa.

## DevTrack 0.1.14

### Correzioni
- **Icona nel Dock su macOS 26:** l'icona è ora nel formato nativo di macOS 26. Non compare più dentro una cornice bianca quando DevTrack è chiuso ed è la stessa quando è aperto.

## DevTrack 0.1.13

### Novità
- **Nuovo logo:** un quadrante con due anelli e un pallino, accanto al nome DevTrack. Cambiano anche l'icona dell'app e quella nella barra dei menu.
- **Logo in movimento:** mentre una sessione è in corso, il pallino gira intorno all'anello. Con "Riduci movimento" attivo il logo resta fermo.

### Correzioni
- **Suggerimenti della descrizione:** i nomi lunghi di attività, progetti e work item di Azure DevOps non escono più dall'elenco, ma finiscono con "…".
- **Finestra compatta:** scorrendo, le sessioni arrivano fino alla linea in fondo, senza più una fascia vuota dove il testo veniva tagliato.

## DevTrack 0.1.12

### Novità
- **Finestra compatta, giorno sempre visibile:** scorrendo le sessioni recenti, la fascia del giorno che stai guardando resta in alto finché la fascia del giorno dopo non prende il suo posto.

### Correzioni
- **Password del portachiavi su macOS:** DevTrack legge il token di Azure DevOps una sola volta per avvio, invece di chiederlo di continuo, e ora è firmato sempre con lo stesso certificato. Al primo avvio di questa versione macOS chiede la password un'ultima volta: scegli "Consenti sempre" e con i prossimi aggiornamenti non dovrebbe più chiederla.
- **Due monitor affiancati:** la finestrella di modifica della finestra compatta si apre a destra, sullo stesso monitor, e non più a cavallo dei due monitor.
- **Passare tra finestra compatta e completa:** la finestra resta dov'è, sullo stesso monitor, invece di tornare dov'era l'ultima volta.
- **Trascinare la finestra:** due trascinamenti veloci non vengono più presi per un doppio clic che allarga la finestra. Nella finestra compatta il doppio clic non la allarga più.

## DevTrack 0.1.11

### Novità
- **Finestra compatta, modifica veloce:** cliccando una sessione conclusa si apre una piccola finestra accanto alla finestra compatta, con attività, progetto, durata, inizio e fine e il giorno. Note, tag e fatturabile restano nella finestra completa. Su Linux con Wayland la stessa modifica si apre dentro la finestra compatta.
- **Finestra compatta, sessione in corso:** attività, progetto e cliente si possono cambiare mentre il timer corre, senza fermarlo. Il pallino verde accanto al tempo sostituisce la riga "In registrazione".
- **Finestra compatta, Impostazioni:** in fondo c'è la stessa voce della barra laterale.

## DevTrack 0.1.10

### Novità
- **Scegliere la data dal campo del periodo:** in Oggi, Rendicontazione e Statistiche la data tra ‹ e › ora si clicca. Puoi scrivere una data (23/11/2026, 23/11 o 2026-11-23) e premere Invio, oppure sceglierla su un piccolo calendario che evidenzia il giorno, la settimana o il mese in vista. La vista resta su Giorno, Settimana o Mese.
- **Finestra compatta più leggibile:** ogni giorno delle sessioni recenti è una fascia colorata con il giorno, la data e il totale, che separa chiaramente un giorno dall'altro.

## DevTrack 0.1.9

### Novità
- **"+ Aggiungi sessione" da ogni pagina:** in Lavoro, Rendicontazione e Statistiche il pulsante in alto apre la finestra sopra la pagina in cui sei, invece di riportarti su Oggi.
- **Work item di Azure DevOps nelle finestre delle sessioni:** in Aggiungi sessione e Modifica sessione l'elenco delle attività mostra anche i work item assegnati a te non ancora in DevTrack; sceglierne uno lo importa e lo seleziona. Un #numero non assegnato a te si può cercare da lì.

### Correzioni
- **"Trova in Azure DevOps…"** nelle finestre delle sessioni non faceva nulla: ora i work item sono nell'elenco stesso.
- **L'elenco delle attività** in quelle finestre non copre più il campo quando è lungo.
- **Calendario:** su un giorno libero senza sessioni (per esempio la domenica nella vista Giorno) DevTrack si chiudeva. Ora il giorno viene mostrato.
- **Timesheet:** le intestazioni dei giorni e i totali in fondo sono allineati alle ore nelle celle.

## DevTrack 0.1.8

### Novità
- **Timesheet più chiaro:** sotto la griglia, "Aggiungi un progetto alla settimana" e "Riempi dalla settimana scorsa" hanno un'icona **i** che spiega cosa fanno; quella della copia dice quante celle riempirà. Aggiunto un progetto, il cursore va nella sua cella di oggi. Una riga senza ore si toglie con ×.

### Correzioni
- **Aggiornamenti:** DevTrack cerca una nuova versione ogni volta che si apre, e ogni 6 ore finché resta aperta. Prima lo faceva al massimo una volta al giorno, quindi una versione appena uscita poteva non essere vista. *Da questa versione in poi; per arrivare alla 0.1.8 usa ancora "Controlla ora" in Impostazioni › Generale.*

## DevTrack 0.1.7

### Novità
- **Avviare una sessione su un work item di Azure DevOps non ancora importato:** nel campo attività di Oggi compaiono anche i work item assegnati a te che non sono ancora in DevTrack, con il progetto in cui andranno. Diventano un'attività quando premi Avvia. Un #numero non assegnato a te si può cercare dai suggerimenti. L'elenco si aggiorna da solo e funziona anche senza rete.
- **Lavoro, ripensato:** i tab sono Attività, Progetti e Clienti, e Lavoro si apre su Attività. Un clic su un'attività la apre sul posto, con ore previste, dettagli e ultime sessioni, e porta al progetto e al cliente con un clic.
- **Stime a colpo d'occhio:** "registrato / previste" anche nel tab Attività, con un **!** quando superi la stima, e un filtro Stime per trovare le attività oltre le ore previste o senza.
- **Tornare indietro come sei arrivato:** il percorso sopra progetti e clienti mostra la strada fatta; ogni passo e Indietro (Alt+←) tornano lì, con la lista com'era.
- **Una sola ricerca:** lo stesso campo in tutte le liste di Lavoro, anche tra i progetti di un cliente. ⌘F per cercare, Esc per svuotare.
- **Aggiungi attività** anche dal tab Attività, scegliendo il progetto.
- **Finestre uniformi:** nuova attività, nuovo progetto e nuovo cliente hanno la stessa finestra e chiedono prima di perdere quello che hai scritto.

## DevTrack 0.1.6

### Correzioni
- **Sfumature delle liste:** in cima e in fondo alle liste non resta più visibile una sottile striscia di righe oltre la sfumatura.

## DevTrack 0.1.5

### Novità
- **Codice aperto:** il codice di DevTrack è pubblico, con licenza MIT, su [github.com/brainupstudio/devtrack](https://github.com/brainupstudio/devtrack), con una guida all'uso.
- **Importazione da un altro strumento di tracciamento:** accanto al token API ora si indica anche l'indirizzo del servizio da cui leggere le voci. Nessuno dei due viene salvato.
- **Formati della durata** con nomi più chiari: "Ore, minuti e secondi", "Minuti e secondi", "Ore decimali".

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
