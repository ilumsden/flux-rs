FROM fluxrm/flux-core:noble-v0.89.0

LABEL maintainer="Vanessasaurus <@vsoch>"

# Match the default user id for a single system so we aren't root
ARG USERNAME=vscode
ARG USER_UID=1000
ARG USER_GID=1000
ENV USERNAME=${USERNAME}
ENV USER_UID=${USER_UID}
ENV USER_GID=${USER_GID}

# Switch back to root
USER root

# Note: Added `sudo` and `curl` to ensure they are available
RUN apt-get update && \
    apt-get install -y libclang-dev sudo curl python3-pip && \
    apt-get clean

# Assuming installing to /usr/local
ENV LD_LIBRARY_PATH=/usr/local/lib

# Install Rust to a system location via rustup
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

# Add the group and user that match our ids
RUN groupadd -g ${USER_GID} ${USERNAME} && \
    adduser --disabled-password --uid ${USER_UID} --gid ${USER_GID} --gecos "" ${USERNAME} && \
    echo "${USERNAME} ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/${USERNAME} && \
    chmod 0440 /etc/sudoers.d/${USERNAME}

RUN mkdir -p ${RUSTUP_HOME} ${CARGO_HOME} && \
    chown -R ${USERNAME}:${USERNAME} ${RUSTUP_HOME} ${CARGO_HOME}

USER $USERNAME

RUN echo '[ -f /usr/local/rustup/.postcreate-ok ] || echo -e "\033[1;31m⚠ postCreate.sh failed or has not finished — see: cat /tmp/postcreate.log\033[0m"' >> /home/${USERNAME}/.bashrc
