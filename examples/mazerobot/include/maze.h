#ifndef MAZE_H
#define MAZE_H

#include <stdbool.h>

#define MAZE_MAX_SIZE 50

typedef enum {
    CELL_WALL = 0,
    CELL_PATH = 1,
    CELL_START = 2,
    CELL_END = 3,
    CELL_VISITED = 4
} CellType;

typedef struct {
    int rows;
    int cols;
    CellType grid[MAZE_MAX_SIZE][MAZE_MAX_SIZE];
    int start_row;
    int start_col;
    int end_row;
    int end_col;
} Maze;

void maze_init(Maze *maze, int rows, int cols);
void maze_generate(Maze *maze, unsigned int seed);
void maze_print(const Maze *maze);
bool maze_is_valid_move(const Maze *maze, int row, int col);
void maze_set_start(Maze *maze, int row, int col);
void maze_set_end(Maze *maze, int row, int col);

#endif
