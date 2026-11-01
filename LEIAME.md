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

## Usar (versão 1.1: tudo automático)
Abra o AudioBridge uma vez em cada aparelho. Dali em diante:
- **Windows:** começa a transmitir sozinho e fica na bandeja (ícones ocultos, perto do relógio). Inicia junto com o Windows.
  Fechar a janela só esconde; para **encerrar de verdade**: botão direito no ícone → **Encerrar**. Clique no ícone para abrir a janela.
- **Mac:** conecta sozinho ao PC e toca o áudio. Inicia junto com o Mac, sem janela e sem ícone no Dock.
  Fechar a janela (bolinha vermelha) mantém tudo rodando em segundo plano. Para abrir a janela de novo, abra o app AudioBridge.
  Para encerrar de vez: abra a janela e use Cmd+Q.
- Os dois lados têm a caixa “Abrir ao ligar…” para desativar o início automático.

Tudo que toca no Windows sai no alto-falante/fone atual do Mac; volume no slider do Mac.
Dica: no Windows, deixe a saída em 48000 Hz e use Wi-Fi 5 GHz ou cabo.

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
