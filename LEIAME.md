# AudioBridge — áudio do Windows 11 no Mac, em tempo real

Pasta `dist/`:
- `AudioBridge.exe` (+ `AudioBridge-Windows.zip`) — Windows 11 x64
- `AudioBridge.app` (+ `AudioBridge-Mac.zip`) — Mac Apple Silicon, macOS 15+

## Instalar
**Windows:** copie `AudioBridge.exe` para qualquer pasta e dê dois cliques. Na 1ª vez o Firewall pergunta:
marque **Redes privadas** e clique **Permitir acesso** (necessário para o Mac achar e falar com o PC).
Se o SmartScreen avisar (app sem assinatura): *Mais informações → Executar assim mesmo*.

**Mac:** arraste `AudioBridge.app` para Aplicativos e abra. Se o macOS bloquear (app sem assinatura de desenvolvedor):
clique direito → *Abrir* → *Abrir*. Na 1ª vez, permita **“encontrar dispositivos na rede local”**.
Não precisa de permissão de microfone/gravação de tela.

## Usar
1. Abra o AudioBridge no Windows e no Mac (mesma rede Wi-Fi/cabo; sem internet necessária).
2. No Mac, o PC aparece sozinho e conecta automaticamente (ou clique **Conectar**).
3. No Windows, clique **Iniciar transmissão**.
4. Tudo que toca no Windows sai no alto-falante/fone atual do Mac. Volume no slider do Mac.

O Mac segue a saída de áudio padrão (trocou de fone, o som acompanha). O Windows captura a saída padrão dele.
Dica: no Windows, deixe a saída em 48000 Hz (Som → Propriedades → Avançado) e use Wi-Fi 5 GHz ou cabo.

## Como funciona
Windows: WASAPI loopback → Opus 160 kbps, quadros de 10 ms, estéreo 48 kHz → UDP.
Mac: UDP → Opus → buffer adaptativo (começa em 40 ms, aumenta sozinho se houver falhas, até 160 ms; corrige
deriva de relógio e esconde perdas de pacote). Descoberta por Bonjour/mDNS. Latência típica: ~60–100 ms.

## Se o Mac não achar o PC
Redes de visitantes/roteadores com “isolamento de clientes” bloqueiam descoberta. Na janela do Mac, abra
“Não achou? Digite o IP do PC” e informe o IP mostrado na janela do Windows.

## Recompilar
`./scripts/build-mac.sh` e `./scripts/build-windows.sh` (precisa de `brew install rustup cmake mingw-w64`).
`testsend`/`testrecv` são ferramentas de teste sem interface.
