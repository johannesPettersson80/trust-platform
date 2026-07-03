import React from "react";
import { Group, Line, Text } from "react-konva";

interface RungProps {
  y: number;
  leftX: number;
  rightX: number;
  rungNumber: number;
  isActive?: boolean;
}

export function Rung({ y, leftX, rightX, rungNumber, isActive = false }: RungProps) {
  return (
    <Group>
      {/* Rung number label */}
      <Text
        text={`${rungNumber}`}
        x={10}
        y={y - 5}
        fontSize={12}
        fill="#666"
        fontFamily="monospace"
      />
      
      {/* Horizontal rung line */}
      <Line
        points={[leftX, y, rightX, y]}
        stroke={isActive ? "orange" : "#333"}
        strokeWidth={isActive ? 3 : 2}
      />
      
      {/* Left power rail connection */}
      <Line
        points={[leftX - 10, y - 50, leftX - 10, y + 50]}
        stroke="#333"
        strokeWidth={4}
      />
      
      {/* Right power rail connection */}
      <Line
        points={[rightX + 10, y - 50, rightX + 10, y + 50]}
        stroke="#333"
        strokeWidth={4}
      />
    </Group>
  );
}
