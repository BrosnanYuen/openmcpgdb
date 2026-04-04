CC = gcc
CFLAGS = -Wall -Wextra -std=c99 -Iinclude -g -O0 -DDEBUG
SRCDIR = src
BUILDDIR = build
TARGET = maze_robot

SRCS = $(SRCDIR)/main.c $(SRCDIR)/maze.c $(SRCDIR)/simulator.c $(SRCDIR)/robot.c $(SRCDIR)/fsm.c
OBJS = $(BUILDDIR)/main.o $(BUILDDIR)/maze.o $(BUILDDIR)/simulator.o $(BUILDDIR)/robot.o $(BUILDDIR)/fsm.o

.PHONY: all clean run debug

all: $(TARGET)

$(BUILDDIR):
	mkdir -p $(BUILDDIR)

$(BUILDDIR)/%.o: $(SRCDIR)/%.c | $(BUILDDIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(TARGET): $(OBJS)
	$(CC) $(CFLAGS) $(OBJS) -o $(TARGET)

run: $(TARGET)
	./$(TARGET)

debug: $(TARGET)
	gdb ./$(TARGET)

clean:
	rm -rf $(BUILDDIR) $(TARGET)
