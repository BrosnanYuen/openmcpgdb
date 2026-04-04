#ifndef SIMULATOR_H
#define SIMULATOR_H

#include "maze.h"

typedef struct {
    Maze maze;
    int robot_row;
    int robot_col;
    int steps;
    bool solved;
} Simulator;

void simulator_init(Simulator *sim, int rows, int cols, unsigned int seed);
bool simulator_move_robot(Simulator *sim, int new_row, int new_col);
void simulator_reset(Simulator *sim);
void simulator_print_status(const Simulator *sim);
void simulator_print(const Simulator *sim);
bool simulator_is_at_goal(const Simulator *sim);

#endif
