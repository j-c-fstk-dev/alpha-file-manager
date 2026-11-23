# Alpha File Manager

[English](README.md)

Gerenciador de arquivos para Linux escrito em Rust com GTK 3. A interface usa componentes
nativos e funciona sem WebView.

![Montanhas e aurora no banner padrão do Alpha File Manager](assets/banner.jpg)

## Recursos

- Navegação em abas e painel duplo com barra de endereço e histórico por painel.
- Barra lateral recolhível com pastas padrão e atalhos fixados, compartilhados com a tela inicial.
- Banner panorâmico personalizável pelo botão de imagem na tela inicial.
- Miniaturas de imagens visíveis, ícones por tipo e ordenação por nome, tamanho, tipo e datas.
- Menus de contexto para propriedades, cópia, movimentação, renomeação e lixeira.
- Perfis verde fósforo, âmbar, ciano e violeta, com opção de transparência nas Configurações.
- Lançador e opção de associação como gerenciador padrão, incluindo integração com o Xfce.

A data de criação aparece como **—** quando o sistema de arquivos não fornece essa informação.
A translucidez da janela depende do compositor. Em outros ambientes, a opção mantém o efeito
visual dentro da interface.

## Instalação

### Pacote de release para Linux x86-64

Extraia o arquivo `alpha-file-manager-v0.2.0-linux-x86_64.tar.gz` e, na pasta extraída, execute:

```sh
./install-launcher.sh
```

O pacote foi compilado no Kali Rolling. Ele exige as bibliotecas de execução do GTK 3 e uma
glibc 2.34 ou posterior. Em distribuições mais antigas, compile a partir do código-fonte.

### Código-fonte

Instale o Rust, o compilador do sistema, o pkg-config e os cabeçalhos do GTK 3. No Kali e no Debian:

```sh
sudo apt install build-essential pkg-config libgtk-3-dev
cargo build --release --locked
./target/release/alpha-file-manager
```

Para instalar no menu de aplicativos da conta atual, rode `./install-launcher.sh`.
Para abrir uma pasta específica, passe seu caminho como argumento.

## Personalização

O idioma segue a configuração do sistema (português ou inglês). A engrenagem também permite
ajustar a opacidade entre 60% e 100% com a transparência ativa. Os menus no cabeçalho
mostram os atalhos disponíveis.

- Use o botão de imagem no banner para escolher outra imagem ou restaurar a original.
- Use a engrenagem para alterar o perfil de cores, ativar transparência ou selecionar o Alpha como padrão.
- Para fixar uma pasta, use **Adicionar pasta** no início ou o menu de contexto na listagem.
  Clique com o botão direito em um atalho para removê-lo.
- Clique nos cabeçalhos da lista para alternar o critério e a direção da ordenação.

## Licença

GNU Affero General Public License, versão 3, conforme o arquivo [LICENSE](LICENSE).
