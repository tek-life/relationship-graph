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
  school?: string | null;
  projects: string[];
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
  school?: string | null;
  projects?: string[];
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
  source: 'manual' | 'inferred' | 'imported';
  confidence?: number | null;
  confirmationStatus: 'confirmed' | 'pending' | 'rejected';
  inferenceReason?: string | null;
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
  edgeSource: 'manual' | 'inferred' | 'imported';
  confirmationStatus: 'confirmed' | 'pending' | 'rejected';
  confidence?: number | null;
  inferenceReason?: string | null;
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

// === NLQ 多意图响应 ===

export type NlqResponse =
  | { intentType: 'searchPeople'; results: NlqResult[] }
  | { intentType: 'createPersonDraft'; draft: PersonDraft }
  | { intentType: 'updatePersonDraft'; draft: UpdateDraft }
  | { intentType: 'addInteractionDraft'; draft: InteractionDraft }
  | { intentType: 'findPath'; path: PathData };

export interface PersonDraft {
  name: string;
  company?: string;
  location?: string;
  title?: string;
  resourceTags: string[];
  background?: string;
  school?: string;
  confidence: number;
}

export interface UpdateDraft {
  targetPerson?: Person;
  candidates: Person[];
  changes: FieldChange[];
  confidence: number;
  errorHint?: string;
}

export interface FieldChange {
  field: string;
  oldValue?: string;
  newValue: string;
}

export interface InteractionDraft {
  personMention: string;
  resolvedPerson?: Person;
  candidates: Person[];
  topic?: string;
  summary?: string;
  actionItems: string[];
  confidence: number;
}

export interface PathData {
  nodes: PathNode[];
  edges: PathEdge[];
  hops: number;
  includesPending: boolean;
  summary: string;
}

export interface PathNode {
  id: string;
  name: string;
  company?: string;
}

export interface PathEdge {
  fromId: string;
  toId: string;
  relationshipType: string;
  strength?: string;
  confirmationStatus: string;
}
