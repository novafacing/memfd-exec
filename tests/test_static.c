#include <netinet/in.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <assert.h>

/* Bind socket to INADDR_ANY, print out the port then print out the data received. */
int main() {
  int sock = socket(AF_INET, SOCK_STREAM, 0);
  struct sockaddr_in addr;
  addr.sin_family = AF_INET;
  addr.sin_port = htons(0);
  addr.sin_addr.s_addr = INADDR_ANY;
  int rc = bind(sock, (struct sockaddr *)&addr, sizeof(addr));
  assert(rc == 0);

  assert(listen(sock, 1) == 0);

  socklen_t addr_len = sizeof(addr);
  getsockname(sock, (struct sockaddr*)&addr, &addr_len);
  assert(addr.sin_port > 0);
  assert(printf("%hu", ntohs(addr.sin_port)) > 0);
  assert(fflush(stdout) == 0);

  int client = accept(sock, NULL, NULL);
  char buf[1024];
  int len = read(client, buf, sizeof(buf));
  assert(write(STDOUT_FILENO, buf, len) > 0);

  assert(close(client) == 0);
  assert(close(sock) == 0);
  return 0;
}
