# Lumo OS — Mandatos do Projeto

## Workflow de Sincronização (CRÍTICO)
- **Regra de Ouro:** O código no hardware real (**Samsung Galaxy Book 4**) deve estar SEMPRE sincronizado com a `master` do GitHub.
- **Passo Obrigatório:** Após qualquer `git push` bem-sucedido, deve-se realizar o `git pull` no hardware via SSH.
- **Integridade:** Nunca fazer alterações diretas no hardware. O hardware é apenas para teste e validação empírica. Todo código nasce no ambiente de desenvolvimento, passa pelos testes, vai para o GitHub e então para o Galaxy.

## Padrões de Teste
- **Testes de Regressão:** Todo bug encontrado deve ser reproduzido em um teste unitário ANTES da correção.
- **Zero Warnings:** O log de compilação e de testes deve estar sempre 100% limpo (Zero Warnings).
- **Validação de Código:** Priorizar validação de lógica pura, métodos e máquinas de estado antes de testes visuais.

## Identidade Visual e UX
- Seguir rigorosamente `docs/UX_GUIDELINES.md`.
- Sem emojis, sem neon, sem cores fora do sRGB calibrado para o Galaxy Book 4.
