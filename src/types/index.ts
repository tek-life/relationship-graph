export type RelationshipStrength = 'strong' | 'medium' | 'weak';
export type SensitivityLevel = 'low' | 'medium' | 'high';
export type PersonStatus = 'follow-up' | 'active' | 'cold';

export interface Person {
  id: string;
  name: string;
  aliases: string[];
  avatar?: string | null;
  phone?: string | null;
  email?: string | null;
  company?: string | null;
  title?: string | null;
  location?: string | null;
  background?: string | null;
  relationshipStrength?: RelationshipStrength | null;
  resourceTags: string[];
  sensitivityLevel: SensitivityLevel;
  status: PersonStatus;
  nextStep?: string | null;
  notes?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreatePersonInput {
  name: string;
  aliases: string[];
  avatar?: string | null;
  phone?: string | null;
  email?: string | null;
  company?: string | null;
  title?: string | null;
  location?: string | null;
  background?: string | null;
  relationshipStrength?: RelationshipStrength | null;
  resourceTags: string[];
  sensitivityLevel: SensitivityLevel;
  status?: PersonStatus;
  nextStep?: string | null;
  notes?: string | null;
}

export interface Relationship {
  id: string;
  fromPersonId: string;
  toPersonId: string;
  relationshipType: 'introduced' | 'colleague' | 'friend' | 'cooperation' | 'other';
  strength?: RelationshipStrength | null;
  description?: string | null;
  createdAt: string;
}

export interface CreateRelationshipInput {
  fromPersonId: string;
  toPersonId: string;
  relationshipType: Relationship['relationshipType'];
  strength?: RelationshipStrength | null;
  description?: string | null;
}

export interface Interaction {
  id: string;
  personId: string;
  timestamp: string;
  content: string;
  summary?: string | null;
  topics: string[];
  actionItems: string[];
  createdAt: string;
}

export interface CreateInteractionInput {
  personId: string;
  timestamp: string;
  content: string;
  summary?: string | null;
  topics: string[];
  actionItems: string[];
}

export interface EntityMention {
  id: string;
  interactionId: string;
  personId?: string | null;
  mentionText: string;
  confidence: number;
  resolved: boolean;
}

export interface CreateEntityMentionInput {
  interactionId: string;
  personId?: string | null;
  mentionText: string;
  confidence: number;
  resolved: boolean;
}

export interface GraphNode {
  id: string;
  label: string;
  sensitivityLevel: SensitivityLevel;
  status: PersonStatus;
}

export interface GraphEdge {
  id: string;
  source: string;
  target: string;
  label: string;
  strength?: RelationshipStrength | null;
}

export interface GraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface NlqResult {
  personId: string;
  displayName: string;
  realNameHidden: boolean;
  sensitivityLevel: SensitivityLevel;
  company?: string | null;
  title?: string | null;
  relationshipStrength?: RelationshipStrength | null;
  lastInteractionSummary?: string | null;
  status: PersonStatus;
  nextStep?: string | null;
  suggestion: string;
}
